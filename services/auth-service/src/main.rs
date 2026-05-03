use tonic::{transport::Server, Request, Response, Status};
use dotenvy::dotenv;
use std::env;
use sqlx::postgres::PgPool;
use uuid::Uuid;
use redis::AsyncCommands;
// NUEVO: Importamos las funciones para encriptar y verificar contraseñas
use bcrypt::{hash, verify, DEFAULT_COST};

pub mod user {
    tonic::include_proto!("arena.user");
}

use user::user_service_server::{UserService, UserServiceServer};
use user::{JoinRequest, JoinResponse};

#[derive(Debug)]
pub struct MyAuthServer {
    pg_pool: PgPool,
    redis_client: redis::Client,
}

#[tonic::async_trait]
impl UserService for MyAuthServer {
    async fn join_arena(
        &self,
        request: Request<JoinRequest>,
    ) -> Result<Response<JoinResponse>, Status> {
        let req = request.into_inner();
        let username = req.username;
        let password = req.password; // Obtenemos la contraseña del nuevo .proto

        println!("📩 Petición de acceso para: {}", username);

        // ====================================================
        // FASE 1: ¿EXISTE EL USUARIO? (PostgreSQL)
        // ====================================================
        // Consultamos si el usuario ya existe para obtener su ID y Hash
        let existing_user: Option<(String, String)> = sqlx::query_as(
            "SELECT id, password_hash FROM users WHERE username = $1"
        )
        .bind(&username)
        .fetch_optional(&self.pg_pool)
        .await
        .map_err(|_| Status::internal("Error al consultar la base de datos"))?;

        let user_id = if let Some((db_id, db_hash)) = existing_user {
            // ➡️ FLUJO: LOGIN (El usuario ya existe)
            println!("🔍 Usuario encontrado. Verificando contraseña...");
            
            let is_valid = verify(&password, &db_hash).unwrap_or(false);
            
            if !is_valid {
                println!("❌ Contraseña incorrecta para: {}", username);
                return Ok(Response::new(JoinResponse {
                    success: false,
                    user_id: "".to_string(),
                    message: "Contraseña incorrecta.".to_string(),
                }));
            }
            
            println!("✅ Login exitoso. Re-creando sesión para: {}", username);
            db_id // Usamos el ID que ya tenía en la base de datos
            
        } else {
            // ➡️ FLUJO: REGISTRO (El usuario no existe)
            println!("🆕 Usuario nuevo detectado. Iniciando SAGA de registro...");
            
            let new_id = Uuid::new_v4().to_string();
            // Encriptamos la contraseña antes de guardarla
            let hashed_pw = hash(&password, DEFAULT_COST).unwrap();

            // PASO 1 DE LA SAGA: Postgres
            let pg_result = sqlx::query(
                "INSERT INTO users (id, username, password_hash, score) VALUES ($1, $2, $3, 0)"
            )
            .bind(&new_id)
            .bind(&username)
            .bind(&hashed_pw)
            .execute(&self.pg_pool)
            .await;

            if let Err(e) = pg_result {
                eprintln!("❌ Error en Postgres: {}", e);
                return Err(Status::internal("Error al registrar el nuevo usuario"));
            }
            println!("✅ Paso 1: Usuario guardado en Postgres con ID: {}", new_id);
            
            new_id // Usamos el nuevo ID generado
        };

        // ====================================================
        // FASE 2: RENOVACIÓN/CREACIÓN DE SESIÓN (Redis)
        // ====================================================
        let mut redis_conn = match self.redis_client.get_async_connection().await {
            Ok(conn) => conn,
            Err(_) => {
                // Si es un registro nuevo y Redis falla, hacemos Rollback. 
                // Si era login, simplemente falla la conexión pero no borramos el usuario.
                println!("⚠️ Redis no disponible.");
                let _ = sqlx::query("DELETE FROM users WHERE id = $1 AND score = 0")
                    .bind(&user_id)
                    .execute(&self.pg_pool)
                    .await;
                return Err(Status::unavailable("Servicio de sesiones no disponible."));
            }
        };

        // Guardamos o actualizamos la sesión (expira en 2 horas)
        let redis_key = format!("session:{}", user_id);
        let redis_res: redis::RedisResult<()> = redis_conn.set_ex(&redis_key, &username, 7200).await;

        match redis_res {
            Ok(_) => {
                println!("✅ Sesión guardada en Redis. ¡Acceso concedido a {}!", username);
                Ok(Response::new(JoinResponse {
                    success: true,
                    user_id, // Devolvemos el ID (ya sea el antiguo o el nuevo)
                    message: "Acceso concedido. Esperando al moderador...".to_string(),
                }))
            }
            Err(e) => {
                eprintln!("❌ Error en Redis: {}", e);
                // Compensación en caso de fallo
                let _ = sqlx::query("DELETE FROM users WHERE id = $1 AND score = 0")
                    .bind(&user_id)
                    .execute(&self.pg_pool)
                    .await;
                
                Ok(Response::new(JoinResponse {
                    success: false,
                    user_id: "".to_string(),
                    message: "Error interno al iniciar sesión.".to_string(),
                }))
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    println!("🏛️ Iniciando Auth-Service (Login/Registro + Saga)...");

    let pg_url = env::var("DATABASE_URL").expect("Falta DATABASE_URL en .env");
    let pg_pool = PgPool::connect(&pg_url).await?;
    println!("✅ Conectado a PostgreSQL");

    let redis_url = env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let redis_client = redis::Client::open(redis_url)?;
    println!("✅ Cliente Redis inicializado");

    // 🚀 MIGRACIÓN AUTOMÁTICA ACTUALIZADA
    // Ahora incluye password_hash y garantiza que username sea único (UNIQUE)
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id VARCHAR(50) PRIMARY KEY,
            username VARCHAR(50) UNIQUE NOT NULL,
            password_hash VARCHAR(255) NOT NULL,
            score INT DEFAULT 0,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );
        "#,
    )
    .execute(&pg_pool)
    .await?;
    println!("✅ Tabla 'users' verificada con esquema de seguridad.");

    let addr = "0.0.0.0:50052".parse().unwrap();
    let auth_server = MyAuthServer { 
        pg_pool, 
        redis_client 
    };

    println!("🚀 Auth-Service escuchando en: {}", addr);

    Server::builder()
        .add_service(UserServiceServer::new(auth_server))
        .serve(addr)
        .await?;

    Ok(())
}