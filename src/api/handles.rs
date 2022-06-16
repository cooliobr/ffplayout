use argon2::{
    password_hash::{rand_core::OsRng, SaltString},
    Argon2, PasswordHasher,
};

use rand::{distributions::Alphanumeric, Rng};
use simplelog::*;
use sqlx::{migrate::MigrateDatabase, sqlite::SqliteQueryResult, Pool, Sqlite, SqlitePool};

use crate::api::utils::GlobalSettings;
use crate::api::{
    models::{Settings, User},
    utils::db_path,
};

#[derive(Debug, sqlx::FromRow)]
struct Role {
    name: String,
}

async fn create_schema() -> Result<SqliteQueryResult, sqlx::Error> {
    let conn = db_connection().await?;
    let query = "PRAGMA foreign_keys = ON;
    CREATE TABLE IF NOT EXISTS global
        (
            id                      INTEGER PRIMARY KEY AUTOINCREMENT,
            secret                  TEXT NOT NULL,
            UNIQUE(secret)
        );
    CREATE TABLE IF NOT EXISTS roles
        (
            id                      INTEGER PRIMARY KEY AUTOINCREMENT,
            name                    TEXT NOT NULL,
            UNIQUE(name)
        );
    CREATE TABLE IF NOT EXISTS presets
        (
            id                      INTEGER PRIMARY KEY AUTOINCREMENT,
            name                    TEXT NOT NULL,
            text                    TEXT NOT NULL,
            x                       TEXT NOT NULL,
            y                       TEXT NOT NULL,
            fontsize                INTEGER NOT NULL DEFAULT 24,
            line_spacing            INTEGER NOT NULL DEFAULT 4,
            fontcolor               TEXT NOT NULL,
            box                     INTEGER NOT NULL DEFAULT 1,
            boxcolor                TEXT NOT NULL,
            boxborderw              INTEGER NOT NULL DEFAULT 4,
            alpha                   TEXT NOT NULL,
            UNIQUE(name)
        );
    CREATE TABLE IF NOT EXISTS settings
        (
            id                      INTEGER PRIMARY KEY AUTOINCREMENT,
            channel_name            TEXT NOT NULL,
            preview_url             TEXT NOT NULL,
            config_path             TEXT NOT NULL,
            extra_extensions        TEXT NOT NULL,
            UNIQUE(channel_name)
        );
    CREATE TABLE IF NOT EXISTS user
        (
            id                      INTEGER PRIMARY KEY AUTOINCREMENT,
            email                   TEXT NOT NULL,
            username                TEXT NOT NULL,
            password                TEXT NOT NULL,
            salt                    TEXT NOT NULL,
            role_id                 INTEGER NOT NULL DEFAULT 2,
            FOREIGN KEY (role_id)   REFERENCES roles (id) ON UPDATE SET NULL ON DELETE SET NULL,
            UNIQUE(email, username)
        );";
    let result = sqlx::query(query).execute(&conn).await;
    conn.close().await;

    result
}

pub async fn db_init() -> Result<&'static str, Box<dyn std::error::Error>> {
    let db_path = db_path()?;

    if !Sqlite::database_exists(&db_path).await.unwrap_or(false) {
        Sqlite::create_database(&db_path).await.unwrap();
        match create_schema().await {
            Ok(_) => info!("Database created Successfully"),
            Err(e) => panic!("{e}"),
        }
    }
    let secret: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(80)
        .map(char::from)
        .collect();

    let instances = db_connection().await?;

    let query = "CREATE TRIGGER global_row_count
        BEFORE INSERT ON global
        WHEN (SELECT COUNT(*) FROM global) >= 1
        BEGIN
            SELECT RAISE(FAIL, 'Database is already init!');
        END;
        INSERT INTO global(secret) VALUES($1);
        INSERT INTO presets(name, text, x, y, fontsize, line_spacing, fontcolor, alpha, box, boxcolor, boxborderw)
            VALUES('Default', 'Wellcome to ffplayout messenger!', '(w-text_w)/2', '(h-text_h)/2', '24', '4', '#ffffff@0xff', '1.0', '0', '#000000@0x80', '4'),
            ('Empty Text', '', '0', '0', '24', '4', '#000000', '0', '0', '#000000', '0'),
            ('Bottom Text fade in', 'The upcoming event will be delayed by a few minutes.', '(w-text_w)/2', '(h-line_h)*0.9', '24', '4', '#ffffff',
                'ifnot(ld(1),st(1,t));if(lt(t,ld(1)+1),0,if(lt(t,ld(1)+2),(t-(ld(1)+1))/1,if(lt(t,ld(1)+8),1,if(lt(t,ld(1)+9),(1-(t-(ld(1)+8)))/1,0))))', '1', '#000000@0x80', '4'),
            ('Scrolling Text', 'We have a very important announcement to make.', 'ifnot(ld(1),st(1,t));if(lt(t,ld(1)+1),w+4,w-w/12*mod(t-ld(1),12*(w+tw)/w))', '(h-line_h)*0.9',
                '24', '4', '#ffffff', '1.0', '1', '#000000@0x80', '4');
        INSERT INTO roles(name) VALUES('admin'), ('user'), ('guest');
        INSERT INTO settings(channel_name, preview_url, config_path, extra_extensions)
        VALUES('Channel 1', 'http://localhost/live/preview.m3u8',
            '/etc/ffplayout/ffplayout.yml', '.jpg,.jpeg,.png');";
    sqlx::query(query).bind(secret).execute(&instances).await?;
    instances.close().await;

    Ok("Database initialized!")
}

pub async fn db_connection() -> Result<Pool<Sqlite>, sqlx::Error> {
    let db_path = db_path().unwrap();
    let conn = SqlitePool::connect(&db_path).await?;

    Ok(conn)
}

pub async fn db_global() -> Result<GlobalSettings, sqlx::Error> {
    let conn = db_connection().await?;
    let query = "SELECT secret FROM global WHERE id = 1";
    let result: GlobalSettings = sqlx::query_as(query).fetch_one(&conn).await?;
    conn.close().await;

    Ok(result)
}

pub async fn db_get_settings(id: &i64) -> Result<Settings, sqlx::Error> {
    let conn = db_connection().await?;
    let query = "SELECT * FROM settings WHERE id = $1";
    let result: Settings = sqlx::query_as(query).bind(id).fetch_one(&conn).await?;
    conn.close().await;

    Ok(result)
}

pub async fn db_update_settings(
    id: i64,
    settings: Settings,
) -> Result<SqliteQueryResult, sqlx::Error> {
    let conn = db_connection().await?;

    let query = "UPDATE settings SET channel_name = $2, preview_url = $3, config_path = $4, extra_extensions = $5 WHERE id = $1";
    let result: SqliteQueryResult = sqlx::query(query)
        .bind(id)
        .bind(settings.channel_name.clone())
        .bind(settings.preview_url.clone())
        .bind(settings.config_path.clone())
        .bind(settings.extra_extensions.clone())
        .execute(&conn)
        .await?;
    conn.close().await;

    Ok(result)
}

pub async fn db_role(id: &i64) -> Result<String, sqlx::Error> {
    let conn = db_connection().await?;
    let query = "SELECT name FROM roles WHERE id = $1";
    let result: Role = sqlx::query_as(query).bind(id).fetch_one(&conn).await?;
    conn.close().await;

    Ok(result.name)
}

pub async fn db_login(user: &str) -> Result<User, sqlx::Error> {
    let conn = db_connection().await?;
    let query = "SELECT id, email, username, password, salt, role_id FROM user WHERE username = $1";
    let result: User = sqlx::query_as(query).bind(user).fetch_one(&conn).await?;
    conn.close().await;

    Ok(result)
}

pub async fn db_add_user(user: User) -> Result<SqliteQueryResult, sqlx::Error> {
    let conn = db_connection().await?;
    let salt = SaltString::generate(&mut OsRng);
    let password_hash = Argon2::default()
        .hash_password(user.password.clone().as_bytes(), &salt)
        .unwrap();

    let query =
        "INSERT INTO user (email, username, password, salt, role_id) VALUES($1, $2, $3, $4, $5)";
    let result = sqlx::query(query)
        .bind(user.email)
        .bind(user.username)
        .bind(password_hash.to_string())
        .bind(salt.to_string())
        .bind(user.role_id)
        .execute(&conn)
        .await?;
    conn.close().await;

    Ok(result)
}

pub async fn db_update_user(id: i64, fields: String) -> Result<SqliteQueryResult, sqlx::Error> {
    let conn = db_connection().await?;
    let query = format!("UPDATE user SET {fields} WHERE id = $1");
    let result: SqliteQueryResult = sqlx::query(&query).bind(id).execute(&conn).await?;
    conn.close().await;

    Ok(result)
}
