package com.arenadepreguntas.client;

import com.arenadepreguntas.grpc.game.*;
import com.arenadepreguntas.grpc.user.*;
import io.grpc.ManagedChannel;
import io.grpc.ManagedChannelBuilder;
import io.grpc.stub.StreamObserver;

import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;
import java.util.function.Consumer;

/**
 * Central gRPC client service. Manages the channel, all stubs, and the
 * bidirectional stream.
 * Designed for both local development (localhost) and Docker deployment.
 */
public class GrpcClientService {

    private static volatile GrpcClientService instance;

    // Configuration — update for different deployment environments
    private static final String GRPC_HOST = "localhost";
    private static final int GRPC_PORT = 8080; // API Gateway port

    private final ManagedChannel channel;
    private final UserServiceGrpc.UserServiceBlockingStub userStub;
    private final GameServiceGrpc.GameServiceStub gameAsyncStub;

    private volatile StreamObserver<ClientMessage> outboundStream;
    private volatile long questionStartTimeMs;

    // Thread-safe handler replacement — swapped when scene transitions
    private final AtomicReference<Consumer<ServerMessage>> messageHandler = new AtomicReference<>(msg -> {
    });

    // ========================================================================
    // Singleton
    // ========================================================================

    public static GrpcClientService getInstance() {
        if (instance == null) {
            synchronized (GrpcClientService.class) {
                if (instance == null) {
                    instance = new GrpcClientService(GRPC_HOST, GRPC_PORT);
                }
            }
        }
        return instance;
    }

    private GrpcClientService(String host, int port) {
        this.channel = ManagedChannelBuilder.forAddress(host, port)
                .usePlaintext()
                .build();
        this.userStub = UserServiceGrpc.newBlockingStub(channel);
        this.gameAsyncStub = GameServiceGrpc.newStub(channel);
    }

    // ========================================================================
    // UserService — Unary (blocking, safe for background threads)
    // ========================================================================

    /**
     * Calls JoinArena and returns the response. Must NOT be called from JavaFX
     * thread.
     */
    public JoinResponse joinArena(String username, String password) {
        JoinRequest req = JoinRequest.newBuilder()
                .setUsername(username)
                .setPassword(password)
                .build();
        return userStub.joinArena(req);
    }

    // ========================================================================
    // GameService — Bidirectional streaming (main game loop)
    // ========================================================================

    /**
     * Opens PlayStream and sets the initial message handler. Call this once after
     * JoinArena succeeds.
     * The handler can be swapped later via setMessageHandler when the scene
     * transitions.
     */
    public void startGameStream(Consumer<ServerMessage> initialHandler) {
        messageHandler.set(initialHandler);

        StreamObserver<ServerMessage> responseObserver = new StreamObserver<ServerMessage>() {
            @Override
            public void onNext(ServerMessage message) {
                // Dispatch to whoever is currently registered
                messageHandler.get().accept(message);
            }

            @Override
            public void onError(Throwable t) {
                System.err.println("[gRPC PlayStream] Error: " + t.getMessage());
                t.printStackTrace();
            }

            @Override
            public void onCompleted() {
                System.out.println("[gRPC PlayStream] Server completed stream.");
            }
        };

        outboundStream = gameAsyncStub.playStream(responseObserver);
    }

    /**
     * Atomically replace the message handler. Safe to call from any thread.
     * Used when LobbyController transitions to ArenaController.
     */
    public void setMessageHandler(Consumer<ServerMessage> handler) {
        if (handler != null) {
            messageHandler.set(handler);
        }
    }

    // ========================================================================
    // Outbound messages — answer and emoji
    // ========================================================================

    /**
     * Record the timestamp of when a question was shown (for response_time_ms
     * calculation).
     */
    public void markQuestionStart() {
        questionStartTimeMs = System.currentTimeMillis();
    }

    /**
     * Send a PlayerResponse with the given answer letter. Calculates
     * response_time_ms.
     */
    public void sendAnswer(String answerLetter) {
        if (outboundStream == null) {
            System.err.println("[gRPC] Stream not open for answer submission");
            return;
        }

        int responseTimeMs = (int) (System.currentTimeMillis() - questionStartTimeMs);

        PlayerResponse response = PlayerResponse.newBuilder()
                .setUserId(SessionData.userId)
                .setAnswer(answerLetter)
                .setResponseTimeMs(responseTimeMs)
                .build();

        ClientMessage msg = ClientMessage.newBuilder()
                .setAnswer(response)
                .build();

        try {
            outboundStream.onNext(msg);
            System.out.println("[gRPC] Sent answer: " + answerLetter + " (time: " + responseTimeMs + "ms)");
        } catch (Exception e) {
            System.err.println("[gRPC] Failed to send answer: " + e.getMessage());
        }
    }

    /**
     * Send an emoji reaction asynchronously (fire-and-forget, no blocking).
     */
    public void sendEmoji(String emojiCode) {
        EmojiRequest request = EmojiRequest.newBuilder()
                .setUserId(SessionData.userId)
                .setEmojiCode(emojiCode)
                .build();

        gameAsyncStub.sendEmoji(request, new StreamObserver<EmojiAck>() {
            @Override
            public void onNext(EmojiAck v) {
            }

            @Override
            public void onError(Throwable t) {
                System.err.println("[gRPC Emoji] " + t.getMessage());
            }

            @Override
            public void onCompleted() {
            }
        });
    }

    // ========================================================================
    // Lifecycle
    // ========================================================================

    public static void shutdownIfInitialized() {
        if (instance != null) {
            instance.shutdown();
        }
    }

    private void shutdown() {
        if (outboundStream != null) {
            try {
                outboundStream.onCompleted();
            } catch (Exception ignored) {
            }
        }
        if (channel != null && !channel.isShutdown()) {
            try {
                channel.shutdown().awaitTermination(5, TimeUnit.SECONDS);
            } catch (InterruptedException e) {
                channel.shutdownNow();
                Thread.currentThread().interrupt();
            }
        }
    }
}
