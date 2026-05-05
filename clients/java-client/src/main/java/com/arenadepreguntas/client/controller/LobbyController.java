package com.arenadepreguntas.client.controller;

import com.arenadepreguntas.client.GrpcClientService;
import com.arenadepreguntas.client.SessionData;
import com.arenadepreguntas.grpc.game.EmojiEvent;
import com.arenadepreguntas.grpc.game.GameStateUpdate;
import com.arenadepreguntas.grpc.game.QuestionPayload;
import com.arenadepreguntas.grpc.game.RoomAccessUpdate;
import com.arenadepreguntas.grpc.game.RoomAccessStatus;

import javafx.animation.FadeTransition;
import javafx.animation.KeyFrame;
import javafx.animation.KeyValue;
import javafx.animation.ParallelTransition;
import javafx.animation.ScaleTransition;
import javafx.animation.Timeline;
import javafx.animation.TranslateTransition;
import javafx.application.Platform;
import javafx.event.ActionEvent;
import javafx.fxml.FXML;
import javafx.fxml.FXMLLoader;
import javafx.geometry.Pos;
import javafx.scene.Node;
import javafx.scene.Parent;
import javafx.scene.Scene;
import javafx.scene.control.Button;
import javafx.scene.control.Label;
import javafx.scene.control.PasswordField;
import javafx.scene.control.TextField;
import javafx.scene.layout.StackPane;
import javafx.scene.layout.VBox;
import javafx.util.Duration;

import java.io.IOException;
import java.util.concurrent.CompletableFuture;

public class LobbyController {

    @FXML
    private VBox entryContainer;
    @FXML
    private VBox waitingContainer;
    @FXML
    private TextField usernameField;
    @FXML
    private PasswordField passwordField;
    @FXML
    private Button playButton;
    @FXML
    private Label waitingLabel;
    @FXML
    private Label usernameDisplay;
    @FXML
    private Label errorLabel;
    @FXML
    private Button emojiBtn1;
    @FXML
    private Button emojiBtn2;
    @FXML
    private Button emojiBtn3;
    @FXML
    private Button emojiBtn4;
    @FXML
    private Button emojiBtn5;
    @FXML
    private StackPane emojiOverlay;

    private Timeline pulseTimeline;

    @FXML
    public void initialize() {
        pulseTimeline = new Timeline();
        pulseTimeline.setCycleCount(Timeline.INDEFINITE);
        if (emojiOverlay != null) {
            emojiOverlay.setPickOnBounds(false);
        }
    }

    // ========================================================================
    // JOIN ARENA
    // ========================================================================

    @FXML
    private void handlePlayClicked(ActionEvent event) {
        String username = usernameField.getText().trim();
        String password = passwordField.getText().trim();

        if (username.isEmpty()) {
            shake(usernameField);
            return;
        }
        if (password.isEmpty()) {
            shake(passwordField);
            return;
        }

        hideError();
        playButton.setDisable(true);
        playButton.setText("Connecting...");

        CompletableFuture
                .supplyAsync(() -> GrpcClientService.getInstance().joinArena(username, password))
                .thenAcceptAsync((response) -> {
                    if (!response.getSuccess()) {
                        showError(response.getMessage().isEmpty()
                                ? "Login failed. Try a different username or password."
                                : response.getMessage());
                        resetPlayButton();
                        return;
                    }

                    SessionData.username = username;
                    SessionData.userId = response.getUserId();

                    entryContainer.setVisible(false);
                    entryContainer.setManaged(false);
                    waitingContainer.setVisible(true);
                    waitingContainer.setManaged(true);
                    usernameDisplay.setText("Playing as: " + username);
                    startPulseAnimation();

                    // Open bidirectional stream. Handler switches when first question arrives.
                    GrpcClientService.getInstance().startGameStream(msg -> {
                        if (msg.hasRoomAccess()) {
                            Platform.runLater(() -> handleRoomAccess(msg.getRoomAccess()));
                        } else if (msg.hasGameState()) {
                            Platform.runLater(() -> handleGameState(msg.getGameState()));
                        } else if (msg.hasNewQuestion()) {
                            // Blank ourselves immediately so subsequent messages go to Arena
                            GrpcClientService.getInstance().setMessageHandler(ignored -> {
                            });
                            Platform.runLater(() -> switchToArena(msg.getNewQuestion()));
                        } else if (msg.hasEmoji()) {
                            Platform.runLater(() -> showEmojiReaction(msg.getEmoji()));
                        }
                    });

                }, Platform::runLater)
                .exceptionally(ex -> {
                    Platform.runLater(() -> {
                        showError("Could not reach the server. Check your connection.");
                        resetPlayButton();
                    });
                    return null;
                });
    }

    // ========================================================================
    // EMOJI REACTIONS
    // ========================================================================

    @FXML
    private void handleEmojiClicked(ActionEvent event) {
        Button clicked = (Button) event.getSource();
        String emoji = (String) clicked.getUserData();

        ScaleTransition up = new ScaleTransition(Duration.millis(75), clicked);
        up.setToX(1.3);
        up.setToY(1.3);
        ScaleTransition down = new ScaleTransition(Duration.millis(75), clicked);
        down.setToX(1.0);
        down.setToY(1.0);
        up.setOnFinished(e -> down.play());
        up.play();

        GrpcClientService.getInstance().sendEmoji(emoji);
    }

    private void showEmojiReaction(EmojiEvent event) {
        if (emojiOverlay == null)
            return;

        Label bubble = new Label(event.getEmojiCode());
        bubble.setStyle("-fx-font-size: 52; -fx-effect: dropshadow(gaussian, rgba(0,0,0,0.5), 8, 0, 0, 2);");
        bubble.setAlignment(Pos.CENTER);

        double xOffset = (Math.random() * 280) - 140;
        bubble.setTranslateX(xOffset);
        bubble.setTranslateY(60);
        bubble.setOpacity(1.0);

        emojiOverlay.getChildren().add(bubble);

        TranslateTransition rise = new TranslateTransition(Duration.millis(2000), bubble);
        rise.setByY(-260);
        FadeTransition fade = new FadeTransition(Duration.millis(2000), bubble);
        fade.setFromValue(1.0);
        fade.setToValue(0.0);
        ParallelTransition anim = new ParallelTransition(rise, fade);
        anim.setOnFinished(e -> emojiOverlay.getChildren().remove(bubble));
        anim.play();
    }

    // ========================================================================
    // Scene transition
    // ========================================================================

    private void switchToArena(QuestionPayload firstQuestion) {
        try {
            FXMLLoader loader = new FXMLLoader(
                    getClass().getResource("/com/arenadepreguntas/client/fxml/arena.fxml"));
            Parent arenaRoot = loader.load();

            ArenaController arenaController = loader.getController();
            arenaController.displayFirstQuestion(firstQuestion);

            Scene scene = waitingContainer.getScene();
            scene.setRoot(arenaRoot);
        } catch (IOException e) {
            e.printStackTrace();
            showError("Failed to load game screen.");
        }
    }

    private void handleRoomAccess(RoomAccessUpdate update) {
        if (!update.getUserId().equals(SessionData.userId)) {
            return;
        }

        if (update.getStatus() == RoomAccessStatus.ROOM_ACCESS_GRANTED) {
            waitingLabel.setText("✅ Access granted. Waiting for the game to start...");
        } else if (update.getStatus() == RoomAccessStatus.ROOM_ACCESS_DENIED) {
            showError(update.getMessage().isEmpty()
                    ? "Access denied by moderator."
                    : update.getMessage());
            waitingContainer.setVisible(false);
            waitingContainer.setManaged(false);
            entryContainer.setVisible(true);
            entryContainer.setManaged(true);
            resetPlayButton();
        } else if (update.getStatus() == RoomAccessStatus.ROOM_ACCESS_PENDING) {
            waitingLabel.setText("⏳ Waiting for moderator approval...");
        }
    }

    private void handleGameState(GameStateUpdate update) {
        if (update.getStarted()) {
            waitingLabel.setText("🎮 Game starting...");
        }
    }

    // ========================================================================
    // Helpers
    // ========================================================================

    private void startPulseAnimation() {
        pulseTimeline.getKeyFrames().clear();
        Duration half = Duration.millis(400);
        Duration full = Duration.millis(800);
        pulseTimeline.getKeyFrames().addAll(
                new KeyFrame(Duration.ZERO, new KeyValue(waitingLabel.opacityProperty(), 1.0)),
                new KeyFrame(half, new KeyValue(waitingLabel.opacityProperty(), 0.5)),
                new KeyFrame(full, new KeyValue(waitingLabel.opacityProperty(), 1.0)));
        pulseTimeline.play();
    }

    private void shake(Node node) {
        TranslateTransition shake = new TranslateTransition(Duration.millis(50), node);
        shake.setCycleCount(4);
        shake.setAutoReverse(true);
        shake.setByX(8);
        shake.play();
    }

    private void showError(String message) {
        errorLabel.setText(message);
        errorLabel.setVisible(true);
        errorLabel.setManaged(true);
    }

    private void hideError() {
        errorLabel.setVisible(false);
        errorLabel.setManaged(false);
    }

    private void resetPlayButton() {
        playButton.setDisable(false);
        playButton.setText("⚡ PLAY!");
    }
}
