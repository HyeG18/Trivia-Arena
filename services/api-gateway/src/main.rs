use tonic::{transport::Channel, transport::Server, Request, Response, Status};
// ¡NUEVO! Importamos esto para poder "limpiar" el stream de datos
use tokio_stream::StreamExt;

pub mod user {
    tonic::include_proto!("arena.user");
}
pub mod game {
    tonic::include_proto!("arena.game");
}

use game::game_service_server::{GameService, GameServiceServer};
use user::user_service_server::{UserService, UserServiceServer};

struct Gateway {
    auth_client_channel: Channel,
    game_client_channel: Channel,
}

#[tonic::async_trait]
impl UserService for Gateway {
    async fn join_arena(
        &self,
        request: Request<user::JoinRequest>,
    ) -> Result<Response<user::JoinResponse>, Status> {
        println!("🔀 Gateway: Redirigiendo JoinArena -> Auth-Service (50052)");
        let mut client =
            user::user_service_client::UserServiceClient::new(self.auth_client_channel.clone());
        client.join_arena(request).await
    }
}

#[tonic::async_trait]
impl GameService for Gateway {
    type PlayStreamStream = tonic::Streaming<game::ServerMessage>;

    async fn play_stream(
        &self,
        request: Request<tonic::Streaming<game::ClientMessage>>,
    ) -> Result<Response<Self::PlayStreamStream>, Status> {
        println!("🔀 Gateway: Redirigiendo Stream -> Game-Engine (50051)");
        let mut client =
            game::game_service_client::GameServiceClient::new(self.game_client_channel.clone());

        // Extraemos los mensajes del stream y descartamos los que tengan errores de red
        let in_stream = request.into_inner();
        let mapped_stream = in_stream.filter_map(|res| res.ok());

        client.play_stream(mapped_stream).await
    }

    async fn send_emoji(
        &self,
        request: Request<game::EmojiRequest>,
    ) -> Result<Response<game::EmojiAck>, Status> {
        println!("🔀 Gateway: Redirigiendo Emoji -> Game-Engine (50051)");
        let mut client =
            game::game_service_client::GameServiceClient::new(self.game_client_channel.clone());
        client.send_emoji(request).await
    }

    async fn launch_question(
        &self,
        request: Request<game::QuestionPayload>,
    ) -> Result<Response<game::ModeratorAck>, Status> {
        println!("🔀 Gateway: Redirigiendo LaunchQuestion -> Game-Engine");
        let mut client =
            game::game_service_client::GameServiceClient::new(self.game_client_channel.clone());
        client.launch_question(request).await
    }

    async fn force_end_timer(
        &self,
        request: Request<game::ForceEndRequest>,
    ) -> Result<Response<game::ModeratorAck>, Status> {
        println!("🔀 Gateway: Redirigiendo ForceEndTimer -> Game-Engine");
        let mut client =
            game::game_service_client::GameServiceClient::new(self.game_client_channel.clone());
        client.force_end_timer(request).await
    }

    async fn approve_player(
        &self,
        request: Request<game::ApprovePlayerRequest>,
    ) -> Result<Response<game::ModeratorAck>, Status> {
        println!("🔀 Gateway: Redirigiendo ApprovePlayer -> Game-Engine");
        let mut client =
            game::game_service_client::GameServiceClient::new(self.game_client_channel.clone());
        client.approve_player(request).await
    }

    async fn deny_player(
        &self,
        request: Request<game::DenyPlayerRequest>,
    ) -> Result<Response<game::ModeratorAck>, Status> {
        println!("🔀 Gateway: Redirigiendo DenyPlayer -> Game-Engine");
        let mut client =
            game::game_service_client::GameServiceClient::new(self.game_client_channel.clone());
        client.deny_player(request).await
    }

    async fn start_game(
        &self,
        request: Request<game::StartGameRequest>,
    ) -> Result<Response<game::ModeratorAck>, Status> {
        println!("🔀 Gateway: Redirigiendo StartGame -> Game-Engine");
        let mut client =
            game::game_service_client::GameServiceClient::new(self.game_client_channel.clone());
        client.start_game(request).await
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // CAMBIO AQUI: Usamos los nombres de los contenedores de Docker
    let auth_channel = Channel::from_static("http://auth-service:50052").connect_lazy();
    let game_channel = Channel::from_static("http://game-engine:50051").connect_lazy();

    let gateway = Gateway {
        auth_client_channel: auth_channel.clone(),
        game_client_channel: game_channel.clone(),
    };

    let addr = "0.0.0.0:8080".parse()?;
    println!("🏛️ API Gateway iniciado en el puerto 8080");
    println!("-> Enrutando Auth a 50052");
    println!("-> Enrutando Juego a 50051");

    Server::builder()
        .add_service(UserServiceServer::new(gateway))
        .add_service(GameServiceServer::new(Gateway {
            auth_client_channel: auth_channel,
            game_client_channel: game_channel,
        }))
        .serve(addr)
        .await?;

    Ok(())
}
