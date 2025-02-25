/// ### Possible endpoints
///
/// Run the API thru the systemd service, or like:
///
/// ```BASH
/// ffpapi -l 127.0.0.1:8787
/// ```
///
/// For all endpoints an (Bearer) authentication is required.\
/// `{id}` represent the channel id, and at default is 1.
use std::{collections::HashMap, env, fs, path::Path};

use actix_multipart::Multipart;
use actix_web::{delete, get, http::StatusCode, patch, post, put, web, HttpResponse, Responder};
use actix_web_grants::{permissions::AuthDetails, proc_macro::has_any_role};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, SaltString},
    Argon2, PasswordHasher, PasswordVerifier,
};
use serde::{Deserialize, Serialize};
use simplelog::*;

use crate::auth::{create_jwt, Claims};
use crate::db::{
    handles,
    models::{Channel, LoginUser, TextPreset, User},
};
use crate::utils::{
    channels::{create_channel, delete_channel},
    control::{control_service, control_state, media_info, send_message, Process},
    errors::ServiceError,
    files::{
        browser, create_directory, remove_file_or_folder, rename_file, upload, MoveObject,
        PathObject,
    },
    playlist::{delete_playlist, generate_playlist, read_playlist, write_playlist},
    playout_config, read_log_file, read_playout_config, Role,
};
use ffplayout_lib::utils::{import::import_file, JsonPlaylist, PlayoutConfig};

#[derive(Serialize)]
struct ResponseObj<T> {
    message: String,
    status: i32,
    data: Option<T>,
}

#[derive(Serialize)]
struct UserObj<T> {
    message: String,
    user: Option<T>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DateObj {
    #[serde(default)]
    date: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct FileObj {
    #[serde(default)]
    path: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ImportObj {
    #[serde(default)]
    file: String,
    #[serde(default)]
    date: String,
}

/// #### User Handling
///
/// **Login**
///
/// ```BASH
/// curl -X POST http://127.0.0.1:8787/auth/login/ -H "Content-Type: application/json" \
/// -d '{ "username": "<USER>", "password": "<PASS>" }'
/// ```
/// **Response:**
///
/// ```JSON
/// {
///     "id": 1,
///     "mail": "user@example.org",
///     "username": "<USER>",
///     "token": "<TOKEN>"
/// }
/// ```
#[post("/auth/login/")]
pub async fn login(credentials: web::Json<User>) -> impl Responder {
    match handles::select_login(&credentials.username).await {
        Ok(mut user) => {
            let pass = user.password.clone();
            let hash = PasswordHash::new(&pass).unwrap();
            user.password = "".into();
            user.salt = None;

            if Argon2::default()
                .verify_password(credentials.password.as_bytes(), &hash)
                .is_ok()
            {
                let role = handles::select_role(&user.role_id.unwrap_or_default())
                    .await
                    .unwrap_or_else(|_| "guest".to_string());
                let claims = Claims::new(user.id, user.username.clone(), role.clone());

                if let Ok(token) = create_jwt(claims) {
                    user.token = Some(token);
                };

                info!("user {} login, with role: {role}", credentials.username);

                web::Json(UserObj {
                    message: "login correct!".into(),
                    user: Some(user),
                })
                .customize()
                .with_status(StatusCode::OK)
            } else {
                error!("Wrong password for {}!", credentials.username);
                web::Json(UserObj {
                    message: "Wrong password!".into(),
                    user: None,
                })
                .customize()
                .with_status(StatusCode::FORBIDDEN)
            }
        }
        Err(e) => {
            error!("Login {} failed! {e}", credentials.username);
            web::Json(UserObj {
                message: format!("Login {} failed!", credentials.username),
                user: None,
            })
            .customize()
            .with_status(StatusCode::BAD_REQUEST)
        }
    }
}

/// From here on all request **must** contain the authorization header:\
/// `"Authorization: Bearer <TOKEN>"`

/// **Get current User**
///
/// ```BASH
/// curl -X GET 'http://127.0.0.1:8787/api/user' -H 'Content-Type: application/json' \
/// -H 'Authorization: Bearer <TOKEN>'
/// ```
#[get("/user")]
#[has_any_role("Role::Admin", "Role::User", type = "Role")]
async fn get_user(user: web::ReqData<LoginUser>) -> Result<impl Responder, ServiceError> {
    match handles::select_user(&user.username).await {
        Ok(user) => Ok(web::Json(user)),
        Err(e) => {
            error!("{e}");
            Err(ServiceError::InternalServerError)
        }
    }
}

/// **Update current User**
///
/// ```BASH
/// curl -X PUT http://127.0.0.1:8787/api/user/1 -H 'Content-Type: application/json' \
/// -d '{"mail": "<MAIL>", "password": "<PASS>"}' -H 'Authorization: <TOKEN>'
/// ```
#[put("/user/{id}")]
#[has_any_role("Role::Admin", "Role::User", type = "Role")]
async fn update_user(
    id: web::Path<i32>,
    user: web::ReqData<LoginUser>,
    data: web::Json<User>,
) -> Result<impl Responder, ServiceError> {
    if id.into_inner() == user.id {
        let mut fields = String::new();

        if let Some(mail) = data.mail.clone() {
            fields.push_str(format!("mail = '{mail}'").as_str());
        }

        if !data.password.is_empty() {
            if !fields.is_empty() {
                fields.push_str(", ");
            }

            let salt = SaltString::generate(&mut OsRng);
            let password_hash = Argon2::default()
                .hash_password(data.password.clone().as_bytes(), &salt)
                .unwrap();

            fields.push_str(format!("password = '{}', salt = '{salt}'", password_hash).as_str());
        }

        if handles::update_user(user.id, fields).await.is_ok() {
            return Ok("Update Success");
        };

        return Err(ServiceError::InternalServerError);
    }

    Err(ServiceError::Unauthorized)
}

/// **Add User**
///
/// ```BASH
/// curl -X POST 'http://127.0.0.1:8787/api/user/' -H 'Content-Type: application/json' \
/// -d '{"mail": "<MAIL>", "username": "<USER>", "password": "<PASS>", "role_id": 1, "channel_id": 1}' \
/// -H 'Authorization: Bearer <TOKEN>'
/// ```
#[post("/user/")]
#[has_any_role("Role::Admin", type = "Role")]
async fn add_user(data: web::Json<User>) -> Result<impl Responder, ServiceError> {
    match handles::insert_user(data.into_inner()).await {
        Ok(_) => Ok("Add User Success"),
        Err(e) => {
            error!("{e}");
            Err(ServiceError::InternalServerError)
        }
    }
}

/// #### ffpapi Settings
///
/// **Get Settings from Channel**
///
/// ```BASH
/// curl -X GET http://127.0.0.1:8787/api/channel/1 -H "Authorization: Bearer <TOKEN>"
/// ```
///
/// **Response:**
///
/// ```JSON
/// {
///     "id": 1,
///     "name": "Channel 1",
///     "preview_url": "http://localhost/live/preview.m3u8",
///     "config_path": "/etc/ffplayout/ffplayout.yml",
///     "extra_extensions": "jpg,jpeg,png",
///     "service": "ffplayout.service",
///     "utc_offset": "+120"
/// }
/// ```
#[get("/channel/{id}")]
#[has_any_role("Role::Admin", "Role::User", type = "Role")]
async fn get_channel(id: web::Path<i32>) -> Result<impl Responder, ServiceError> {
    if let Ok(channel) = handles::select_channel(&id).await {
        return Ok(web::Json(channel));
    }

    Err(ServiceError::InternalServerError)
}

/// **Get settings from all Channels**
///
/// ```BASH
/// curl -X GET http://127.0.0.1:8787/api/channels -H "Authorization: Bearer <TOKEN>"
/// ```
#[get("/channels")]
#[has_any_role("Role::Admin", type = "Role")]
async fn get_all_channels() -> Result<impl Responder, ServiceError> {
    if let Ok(channel) = handles::select_all_channels().await {
        return Ok(web::Json(channel));
    }

    Err(ServiceError::InternalServerError)
}

/// **Update Channel**
///
/// ```BASH
/// curl -X PATCH http://127.0.0.1:8787/api/channel/1 -H "Content-Type: application/json" \
/// -d '{ "id": 1, "name": "Channel 1", "preview_url": "http://localhost/live/stream.m3u8", \
/// "config_path": "/etc/ffplayout/ffplayout.yml", "extra_extensions": "jpg,jpeg,png"}' \
/// -H "Authorization: Bearer <TOKEN>"
/// ```
#[patch("/channel/{id}")]
#[has_any_role("Role::Admin", type = "Role")]
async fn patch_channel(
    id: web::Path<i32>,
    data: web::Json<Channel>,
) -> Result<impl Responder, ServiceError> {
    if handles::update_channel(*id, data.into_inner())
        .await
        .is_ok()
    {
        return Ok("Update Success");
    };

    Err(ServiceError::InternalServerError)
}

/// **Create new Channel**
///
/// ```BASH
/// curl -X POST http://127.0.0.1:8787/api/channel/ -H "Content-Type: application/json" \
/// -d '{ "name": "Channel 2", "preview_url": "http://localhost/live/channel2.m3u8", \
/// "config_path": "/etc/ffplayout/channel2.yml", "extra_extensions": "jpg,jpeg,png",
/// "service": "ffplayout@channel2.service" }' \
/// -H "Authorization: Bearer <TOKEN>"
/// ```
#[post("/channel/")]
#[has_any_role("Role::Admin", type = "Role")]
async fn add_channel(data: web::Json<Channel>) -> Result<impl Responder, ServiceError> {
    match create_channel(data.into_inner()).await {
        Ok(c) => Ok(web::Json(c)),
        Err(e) => Err(e),
    }
}

/// **Delete Channel**
///
/// ```BASH
/// curl -X DELETE http://127.0.0.1:8787/api/channel/2 -H "Authorization: Bearer <TOKEN>"
/// ```
#[delete("/channel/{id}")]
#[has_any_role("Role::Admin", type = "Role")]
async fn remove_channel(id: web::Path<i32>) -> Result<impl Responder, ServiceError> {
    if delete_channel(*id).await.is_ok() {
        return Ok("Delete Channel Success");
    }

    Err(ServiceError::InternalServerError)
}

/// #### ffplayout Config
///
/// **Get Config**
///
/// ```BASH
/// curl -X GET http://127.0.0.1:8787/api/playout/config/1 -H 'Authorization: <TOKEN>'
/// ```
///
/// Response is a JSON object from the ffplayout.yml
#[get("/playout/config/{id}")]
#[has_any_role("Role::Admin", "Role::User", type = "Role")]
async fn get_playout_config(
    id: web::Path<i32>,
    _details: AuthDetails<Role>,
) -> Result<impl Responder, ServiceError> {
    if let Ok(channel) = handles::select_channel(&id).await {
        if let Ok(config) = read_playout_config(&channel.config_path) {
            return Ok(web::Json(config));
        }
    };

    Err(ServiceError::InternalServerError)
}

/// **Update Config**
///
/// ```BASH
/// curl -X PUT http://127.0.0.1:8787/api/playout/config/1 -H "Content-Type: application/json" \
/// -d { <CONFIG DATA> } -H 'Authorization: <TOKEN>'
/// ```
#[put("/playout/config/{id}")]
#[has_any_role("Role::Admin", type = "Role")]
async fn update_playout_config(
    id: web::Path<i32>,
    data: web::Json<PlayoutConfig>,
) -> Result<impl Responder, ServiceError> {
    if let Ok(channel) = handles::select_channel(&id).await {
        if let Ok(f) = std::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&channel.config_path)
        {
            serde_yaml::to_writer(f, &data).unwrap();

            return Ok("Update playout config success.");
        } else {
            return Err(ServiceError::InternalServerError);
        };
    };

    Err(ServiceError::InternalServerError)
}

/// #### Text Presets
///
/// Text presets are made for sending text messages to the ffplayout engine, to overlay them as a lower third.
///
/// **Get all Presets**
///
/// ```BASH
/// curl -X GET http://127.0.0.1:8787/api/presets/ -H 'Content-Type: application/json' \
/// -H 'Authorization: <TOKEN>'
/// ```
#[get("/presets/{id}")]
#[has_any_role("Role::Admin", "Role::User", type = "Role")]
async fn get_presets(id: web::Path<i32>) -> Result<impl Responder, ServiceError> {
    if let Ok(presets) = handles::select_presets(*id).await {
        return Ok(web::Json(presets));
    }

    Err(ServiceError::InternalServerError)
}

/// **Update Preset**
///
/// ```BASH
/// curl -X PUT http://127.0.0.1:8787/api/presets/1 -H 'Content-Type: application/json' \
/// -d '{ "name": "<PRESET NAME>", "text": "<TEXT>", "x": "<X>", "y": "<Y>", "fontsize": 24, \
/// "line_spacing": 4, "fontcolor": "#ffffff", "box": 1, "boxcolor": "#000000", "boxborderw": 4, "alpha": 1.0, "channel_id": 1 }' \
/// -H 'Authorization: <TOKEN>'
/// ```
#[put("/presets/{id}")]
#[has_any_role("Role::Admin", "Role::User", type = "Role")]
async fn update_preset(
    id: web::Path<i32>,
    data: web::Json<TextPreset>,
) -> Result<impl Responder, ServiceError> {
    if handles::update_preset(&id, data.into_inner()).await.is_ok() {
        return Ok("Update Success");
    }

    Err(ServiceError::InternalServerError)
}

/// **Add new Preset**
///
/// ```BASH
/// curl -X POST http://127.0.0.1:8787/api/presets/ -H 'Content-Type: application/json' \
/// -d '{ "name": "<PRESET NAME>", "text": "TEXT>", "x": "<X>", "y": "<Y>", "fontsize": 24, \
/// "line_spacing": 4, "fontcolor": "#ffffff", "box": 1, "boxcolor": "#000000", "boxborderw": 4, "alpha": 1.0, "channel_id": 1 }' \
/// -H 'Authorization: <TOKEN>'
/// ```
#[post("/presets/")]
#[has_any_role("Role::Admin", "Role::User", type = "Role")]
async fn add_preset(data: web::Json<TextPreset>) -> Result<impl Responder, ServiceError> {
    if handles::insert_preset(data.into_inner()).await.is_ok() {
        return Ok("Add preset Success");
    }

    Err(ServiceError::InternalServerError)
}

/// **Delete Preset**
///
/// ```BASH
/// curl -X DELETE http://127.0.0.1:8787/api/presets/1 -H 'Content-Type: application/json' \
/// -H 'Authorization: <TOKEN>'
/// ```
#[delete("/presets/{id}")]
#[has_any_role("Role::Admin", "Role::User", type = "Role")]
async fn delete_preset(id: web::Path<i32>) -> Result<impl Responder, ServiceError> {
    if handles::delete_preset(&id).await.is_ok() {
        return Ok("Delete preset Success");
    }

    Err(ServiceError::InternalServerError)
}

/// ### ffplayout controlling
///
/// here we communicate with the engine for:
/// - jump to last or next clip
/// - reset playlist state
/// - get infos about current, next, last clip
/// - send text to the engine, for overlaying it (as lower third etc.)
///
/// **Send Text to ffplayout**
///
/// ```BASH
/// curl -X POST http://127.0.0.1:8787/api/control/1/text/ \
/// -H 'Content-Type: application/json' -H 'Authorization: <TOKEN>' \
/// -d '{"text": "Hello from ffplayout", "x": "(w-text_w)/2", "y": "(h-text_h)/2", \
///     "fontsize": "24", "line_spacing": "4", "fontcolor": "#ffffff", "box": "1", \
///     "boxcolor": "#000000", "boxborderw": "4", "alpha": "1.0"}'
/// ```
#[post("/control/{id}/text/")]
#[has_any_role("Role::Admin", "Role::User", type = "Role")]
pub async fn send_text_message(
    id: web::Path<i32>,
    data: web::Json<HashMap<String, String>>,
) -> Result<impl Responder, ServiceError> {
    match send_message(*id, data.into_inner()).await {
        Ok(res) => Ok(res.text().await.unwrap_or_else(|_| "Success".into())),
        Err(e) => Err(e),
    }
}

/// **Control Playout**
///
/// - next
/// - back
/// - reset
///
/// ```BASH
/// curl -X POST http://127.0.0.1:8787/api/control/1/playout/ -H 'Content-Type: application/json'
/// -d '{ "command": "reset" }' -H 'Authorization: <TOKEN>'
/// ```
#[post("/control/{id}/playout/")]
#[has_any_role("Role::Admin", "Role::User", type = "Role")]
pub async fn control_playout(
    id: web::Path<i32>,
    control: web::Json<Process>,
) -> Result<impl Responder, ServiceError> {
    match control_state(*id, control.command.clone()).await {
        Ok(res) => Ok(res.text().await.unwrap_or_else(|_| "Success".into())),
        Err(e) => Err(e),
    }
}

/// **Get current Clip**
///
/// ```BASH
/// curl -X GET http://127.0.0.1:8787/api/control/1/media/current
/// -H 'Content-Type: application/json' -H 'Authorization: <TOKEN>'
/// ```
///
/// **Response:**
///
/// ```JSON
/// {
///     "jsonrpc": "2.0",
///     "result": {
///       "current_media": {
///         "category": "",
///         "duration": 154.2,
///         "out": 154.2,
///         "seek": 0.0,
///         "source": "/opt/tv-media/clip.mp4"
///       },
///       "index": 39,
///       "play_mode": "playlist",
///       "played_sec": 67.80771999300123,
///       "remaining_sec": 86.39228000699876,
///       "start_sec": 24713.631999999998,
///       "start_time": "06:51:53.631"
///     },
///     "id": 1
/// }
/// ```
#[get("/control/{id}/media/current")]
#[has_any_role("Role::Admin", "Role::User", type = "Role")]
pub async fn media_current(id: web::Path<i32>) -> Result<impl Responder, ServiceError> {
    match media_info(*id, "current".into()).await {
        Ok(res) => Ok(res.text().await.unwrap_or_else(|_| "Success".into())),
        Err(e) => Err(e),
    }
}

/// **Get next Clip**
///
/// ```BASH
/// curl -X GET http://127.0.0.1:8787/api/control/1/media/next/ -H 'Authorization: <TOKEN>'
/// ```
#[get("/control/{id}/media/next")]
#[has_any_role("Role::Admin", "Role::User", type = "Role")]
pub async fn media_next(id: web::Path<i32>) -> Result<impl Responder, ServiceError> {
    match media_info(*id, "next".into()).await {
        Ok(res) => Ok(res.text().await.unwrap_or_else(|_| "Success".into())),
        Err(e) => Err(e),
    }
}

/// **Get last Clip**
///
/// ```BASH
/// curl -X GET http://127.0.0.1:8787/api/control/1/media/last/
/// -H 'Content-Type: application/json' -H 'Authorization: <TOKEN>'
/// ```
#[get("/control/{id}/media/last")]
#[has_any_role("Role::Admin", "Role::User", type = "Role")]
pub async fn media_last(id: web::Path<i32>) -> Result<impl Responder, ServiceError> {
    match media_info(*id, "last".into()).await {
        Ok(res) => Ok(res.text().await.unwrap_or_else(|_| "Success".into())),
        Err(e) => Err(e),
    }
}

/// #### ffplayout Process Control
///
/// Control ffplayout process, like:
/// - start
/// - stop
/// - restart
/// - status
///
/// ```BASH
/// curl -X POST http://127.0.0.1:8787/api/control/1/process/
/// -H 'Content-Type: application/json' -H 'Authorization: <TOKEN>'
/// -d '{"command": "start"}'
/// ```
#[post("/control/{id}/process/")]
#[has_any_role("Role::Admin", "Role::User", type = "Role")]
pub async fn process_control(
    id: web::Path<i32>,
    proc: web::Json<Process>,
) -> Result<impl Responder, ServiceError> {
    control_service(*id, &proc.command).await
}

/// #### ffplayout Playlist Operations
///
/// **Get playlist**
///
/// ```BASH
/// curl -X GET http://127.0.0.1:8787/api/playlist/1?date=2022-06-20
/// -H 'Content-Type: application/json' -H 'Authorization: <TOKEN>'
/// ```
#[get("/playlist/{id}")]
#[has_any_role("Role::Admin", "Role::User", type = "Role")]
pub async fn get_playlist(
    id: web::Path<i32>,
    obj: web::Query<DateObj>,
) -> Result<impl Responder, ServiceError> {
    match read_playlist(*id, obj.date.clone()).await {
        Ok(playlist) => Ok(web::Json(playlist)),
        Err(e) => Err(e),
    }
}

/// **Save playlist**
///
/// ```BASH
/// curl -X POST http://127.0.0.1:8787/api/playlist/1/
/// -H 'Content-Type: application/json' -H 'Authorization: <TOKEN>'
/// -- data "{<JSON playlist data>}"
/// ```
#[post("/playlist/{id}/")]
#[has_any_role("Role::Admin", "Role::User", type = "Role")]
pub async fn save_playlist(
    id: web::Path<i32>,
    data: web::Json<JsonPlaylist>,
) -> Result<impl Responder, ServiceError> {
    match write_playlist(*id, data.into_inner()).await {
        Ok(res) => Ok(res),
        Err(e) => Err(e),
    }
}

/// **Generate Playlist**
///
/// A new playlist will be generated and response.
///
/// ```BASH
/// curl -X GET http://127.0.0.1:8787/api/playlist/1/generate/2022-06-20
/// -H 'Content-Type: application/json' -H 'Authorization: <TOKEN>'
/// ```
#[get("/playlist/{id}/generate/{date}")]
#[has_any_role("Role::Admin", "Role::User", type = "Role")]
pub async fn gen_playlist(
    params: web::Path<(i32, String)>,
) -> Result<impl Responder, ServiceError> {
    match generate_playlist(params.0, params.1.clone()).await {
        Ok(playlist) => Ok(web::Json(playlist)),
        Err(e) => Err(e),
    }
}

/// **Delete Playlist**
///
/// ```BASH
/// curl -X DELETE http://127.0.0.1:8787/api/playlist/1/2022-06-20
/// -H 'Content-Type: application/json' -H 'Authorization: <TOKEN>'
/// ```
#[delete("/playlist/{id}/{date}")]
#[has_any_role("Role::Admin", "Role::User", type = "Role")]
pub async fn del_playlist(
    params: web::Path<(i32, String)>,
) -> Result<impl Responder, ServiceError> {
    match delete_playlist(params.0, &params.1).await {
        Ok(_) => Ok(format!("Delete playlist from {} success!", params.1)),
        Err(e) => Err(e),
    }
}

/// ### Log file
///
/// **Read Log Life**
///
/// ```BASH
/// curl -X Get http://127.0.0.1:8787/api/log/1
/// -H 'Content-Type: application/json' -H 'Authorization: <TOKEN>'
/// ```
#[get("/log/{id}")]
#[has_any_role("Role::Admin", "Role::User", type = "Role")]
pub async fn get_log(
    id: web::Path<i32>,
    log: web::Query<DateObj>,
) -> Result<impl Responder, ServiceError> {
    read_log_file(&id, &log.date).await
}

/// ### File Operations
///
/// **Get File/Folder List**
///
/// ```BASH
/// curl -X POST http://127.0.0.1:8787/api/file/1/browse/ -H 'Content-Type: application/json'
/// -d '{ "source": "/" }' -H 'Authorization: <TOKEN>'
/// ```
#[post("/file/{id}/browse/")]
#[has_any_role("Role::Admin", "Role::User", type = "Role")]
pub async fn file_browser(
    id: web::Path<i32>,
    data: web::Json<PathObject>,
) -> Result<impl Responder, ServiceError> {
    match browser(*id, &data.into_inner()).await {
        Ok(obj) => Ok(web::Json(obj)),
        Err(e) => Err(e),
    }
}

/// **Create Folder**
///
/// ```BASH
/// curl -X POST http://127.0.0.1:8787/api/file/1/create-folder/ -H 'Content-Type: application/json'
/// -d '{"source": "<FOLDER PATH>"}' -H 'Authorization: <TOKEN>'
/// ```
#[post("/file/{id}/create-folder/")]
#[has_any_role("Role::Admin", "Role::User", type = "Role")]
pub async fn add_dir(
    id: web::Path<i32>,
    data: web::Json<PathObject>,
) -> Result<HttpResponse, ServiceError> {
    create_directory(*id, &data.into_inner()).await
}

/// **Rename File**
///
/// ```BASH
/// curl -X POST http://127.0.0.1:8787/api/file/1/rename/ -H 'Content-Type: application/json'
/// -d '{"source": "<SOURCE>", "target": "<TARGET>"}' -H 'Authorization: <TOKEN>'
/// ```
#[post("/file/{id}/rename/")]
#[has_any_role("Role::Admin", "Role::User", type = "Role")]
pub async fn move_rename(
    id: web::Path<i32>,
    data: web::Json<MoveObject>,
) -> Result<impl Responder, ServiceError> {
    match rename_file(*id, &data.into_inner()).await {
        Ok(obj) => Ok(web::Json(obj)),
        Err(e) => Err(e),
    }
}

/// **Remove File/Folder**
///
/// ```BASH
/// curl -X POST http://127.0.0.1:8787/api/file/1/remove/ -H 'Content-Type: application/json'
/// -d '{"source": "<SOURCE>"}' -H 'Authorization: <TOKEN>'
/// ```
#[post("/file/{id}/remove/")]
#[has_any_role("Role::Admin", "Role::User", type = "Role")]
pub async fn remove(
    id: web::Path<i32>,
    data: web::Json<PathObject>,
) -> Result<impl Responder, ServiceError> {
    match remove_file_or_folder(*id, &data.into_inner().source).await {
        Ok(obj) => Ok(web::Json(obj)),
        Err(e) => Err(e),
    }
}

/// **Upload File**
///
/// ```BASH
/// curl -X POST http://127.0.0.1:8787/api/file/1/upload/ -H 'Authorization: <TOKEN>'
/// -F "file=@file.mp4"
/// ```
#[put("/file/{id}/upload/")]
#[has_any_role("Role::Admin", "Role::User", type = "Role")]
async fn save_file(
    id: web::Path<i32>,
    payload: Multipart,
    obj: web::Query<FileObj>,
) -> Result<HttpResponse, ServiceError> {
    upload(*id, payload, &obj.path, false).await
}

/// **Import playlist**
///
/// Import text/m3u file and convert it to a playlist
/// lines with leading "#" will be ignore
///
/// ```BASH
/// curl -X POST http://127.0.0.1:8787/api/file/1/import/ -H 'Authorization: <TOKEN>'
/// -F "file=@list.m3u"
/// ```
#[put("/file/{id}/import/")]
#[has_any_role("Role::Admin", "Role::User", type = "Role")]
async fn import_playlist(
    id: web::Path<i32>,
    payload: Multipart,
    obj: web::Query<ImportObj>,
) -> Result<HttpResponse, ServiceError> {
    let file = Path::new(&obj.file).file_name().unwrap_or_default();
    let path = env::temp_dir().join(&file).to_string_lossy().to_string();
    let (config, _) = playout_config(&id).await?;
    let channel = handles::select_channel(&id).await?;

    upload(*id, payload, &path, true).await?;
    import_file(&config, &obj.date, Some(channel.name), &path)?;

    fs::remove_file(path)?;

    Ok(HttpResponse::Ok().into())
}
