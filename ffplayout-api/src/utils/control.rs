use std::{collections::HashMap, process::Command};

use reqwest::{
    header::{HeaderMap, AUTHORIZATION, CONTENT_TYPE},
    Client, Response,
};
use serde::{Deserialize, Serialize};

use crate::db::handles::select_channel;
use crate::utils::{errors::ServiceError, playout_config};
use ffplayout_lib::vec_strings;

#[derive(Debug, Deserialize, Serialize, Clone)]
struct RpcObj<T> {
    jsonrpc: String,
    id: i32,
    method: String,
    params: T,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct TextParams {
    control: String,
    message: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct ControlParams {
    control: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct MediaParams {
    media: String,
}

impl<T> RpcObj<T> {
    fn new(id: i32, method: String, params: T) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            method,
            params,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Process {
    pub command: String,
}

struct SystemD {
    service: String,
    cmd: Vec<String>,
}

impl SystemD {
    async fn new(id: i32) -> Result<Self, ServiceError> {
        let channel = select_channel(&id).await?;

        Ok(Self {
            service: channel.service,
            cmd: vec_strings!["/usr/bin/systemctl"],
        })
    }

    fn enable(mut self) -> Result<String, ServiceError> {
        self.cmd
            .append(&mut vec!["enable".to_string(), self.service]);

        Command::new("sudo").args(self.cmd).spawn()?;

        Ok("Success".to_string())
    }

    fn disable(mut self) -> Result<String, ServiceError> {
        self.cmd
            .append(&mut vec!["disable".to_string(), self.service]);

        Command::new("sudo").args(self.cmd).spawn()?;

        Ok("Success".to_string())
    }

    fn start(mut self) -> Result<String, ServiceError> {
        self.cmd
            .append(&mut vec!["start".to_string(), self.service]);

        Command::new("sudo").args(self.cmd).spawn()?;

        Ok("Success".to_string())
    }

    fn stop(mut self) -> Result<String, ServiceError> {
        self.cmd.append(&mut vec!["stop".to_string(), self.service]);

        Command::new("sudo").args(self.cmd).spawn()?;

        Ok("Success".to_string())
    }

    fn restart(mut self) -> Result<String, ServiceError> {
        self.cmd
            .append(&mut vec!["restart".to_string(), self.service]);

        Command::new("sudo").args(self.cmd).spawn()?;

        Ok("Success".to_string())
    }

    fn status(mut self) -> Result<String, ServiceError> {
        self.cmd
            .append(&mut vec!["is-active".to_string(), self.service]);

        let output = Command::new("sudo").args(self.cmd).output()?;

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}

fn create_header(auth: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        "Content-Type: application/json".parse().unwrap(),
    );
    headers.insert(AUTHORIZATION, auth.parse().unwrap());

    headers
}

async fn post_request<T>(id: i32, obj: RpcObj<T>) -> Result<Response, ServiceError>
where
    T: Serialize,
{
    let (config, _) = playout_config(&id).await?;
    let url = format!("http://{}", config.rpc_server.address);
    let client = Client::new();

    match client
        .post(&url)
        .headers(create_header(&config.rpc_server.authorization))
        .json(&obj)
        .send()
        .await
    {
        Ok(result) => Ok(result),
        Err(e) => Err(ServiceError::ServiceUnavailable(e.to_string())),
    }
}

pub async fn send_message(
    id: i32,
    message: HashMap<String, String>,
) -> Result<Response, ServiceError> {
    let json_obj = RpcObj::new(
        id,
        "player".into(),
        TextParams {
            control: "text".into(),
            message,
        },
    );

    post_request(id, json_obj).await
}

pub async fn control_state(id: i32, command: String) -> Result<Response, ServiceError> {
    let json_obj = RpcObj::new(id, "player".into(), ControlParams { control: command });

    post_request(id, json_obj).await
}

pub async fn media_info(id: i32, command: String) -> Result<Response, ServiceError> {
    let json_obj = RpcObj::new(id, "player".into(), MediaParams { media: command });

    post_request(id, json_obj).await
}

pub async fn control_service(id: i32, command: &str) -> Result<String, ServiceError> {
    let system_d = SystemD::new(id).await?;

    match command {
        "enable" => system_d.enable(),
        "disable" => system_d.disable(),
        "start" => system_d.start(),
        "stop" => system_d.stop(),
        "restart" => system_d.restart(),
        "status" => system_d.status(),
        _ => Err(ServiceError::BadRequest("Command not found!".to_string())),
    }
}
