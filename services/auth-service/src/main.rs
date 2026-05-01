use tonic::{transport::Server, Request, Response, Status};
use dotenvy::dotenv;
use std::env;
use sqlx::postgres::PgPool;
use uuid::Uuid;

// Importamos el código generado por tonic basado en tu user.proto
pub mod user {
    tonic::include_proto!("arena.user");
}

use user::user_service_server::{UserService, UserServiceServer};
use user::{JoinRequest, JoinResponse};

#[derive(Debug)]
pub struct MyAuthServer {
    // ESTE SERVICIO SOLO SE CONECTA A POSTGRESQL (Database per Service)
    pg_pool: PgPool,
}

#[tonic::async_trait]
impl UserService for MyAuthServer {
    async fn join_arena(
        &self,
        request: Request<JoinRequest>,
    ) -> Result<Response<JoinResponse>, Status> {
        let req = request.into_inner();
        let username = req.username;

        println!("📩 Recibida petición de ingreso para el usuario: {}", username);

        // 1. Generamos un ID único universal (UUID) para la sesión del jugador
        let user_id = Uuid::new_v4().to_string();

        // 2. Insertamos el usuario en PostgreSQL usando .bind() (Evaluación en tiempo de ejecución)
        let result = sqlx::query(
            "INSERT INTO users (id, username, score) VALUES ($1, $2, 0)"
        )
        .bind(&user_id)
        .bind(&username)
        .execute(&self.pg_pool)
        .await;

        match result {
            Ok(_) => {
                println!("✅ Jugador registrado en PostgreSQL con ID: {}", user_id);
                // 3. Devolvemos la respuesta exitosa al cliente Java
                Ok(Response::new(JoinResponse {
                    success: true,
                    user_id,
                    message: "Esperando al moderador...".to_string(),
                }))
            }
            Err(e) => {
                eprintln!("❌ Error al guardar en base de datos: {}", e);
                // Si falla la BD, le avisamos al cliente
                Ok(Response::new(JoinResponse {
                    success: false,
                    user_id: "".to_string(),
                    message: "Error interno del servidor. Intenta de nuevo.".to_string(),
                }))
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Cargamos las variables de entorno (buscando el .env dos niveles arriba)
    dotenv().ok();

    println!("Iniciando Auth-Service...");

    // 2. Conectamos a PostgreSQL
    let pg_url = env::var("DATABASE_URL").expect("Falta DATABASE_URL en .env");
    let pg_pool = PgPool::connect(&pg_url).await?;
    println!("✅ Conectado a PostgreSQL");

    // 3. MIGRACIÓN AUTOMÁTICA: Creamos la tabla 'users' si no existe
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id VARCHAR(50) PRIMARY KEY,
            username VARCHAR(50) NOT NULL,
            score INT DEFAULT 0,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );
        "#,
    )
    .execute(&pg_pool)
    .await?;
    println!("✅ Tabla 'users' verificada en la base de datos.");

    // 4. INICIAMOS EL SERVIDOR gRPC EN EL PUERTO 50052
    // Nota: El Game-Engine usa el 50051, este debe usar otro puerto libre
    let addr = "0.0.0.0:50052".parse().unwrap();
    let auth_server = MyAuthServer { pg_pool };

    println!("🚀 Auth-Service escuchando peticiones en: {}", addr);

    Server::builder()
        .add_service(UserServiceServer::new(auth_server))
        .serve(addr)
        .await?;

    Ok(())
}
