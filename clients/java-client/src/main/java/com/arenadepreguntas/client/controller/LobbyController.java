package com.arenadepreguntas.client.controller;

import javafx.animation.ScaleTransition;
import javafx.animation.Timeline;
import javafx.animation.TranslateTransition;
import javafx.application.Platform;
import javafx.event.ActionEvent;
import javafx.fxml.FXML;
import javafx.scene.control.Button;
import javafx.scene.control.Label;
import javafx.scene.control.TextField;
import javafx.scene.layout.VBox;
import javafx.util.Duration;

import com.arenadepreguntas.client.SessionData;

public class LobbyController {
    @FXML
    private VBox entryContainer;
    @FXML
    private VBox waitingContainer;
    @FXML
    private TextField usernameField;
    @FXML
    private Button playButton;
    @FXML
    private Label waitingLabel;
    @FXML
    private Label usernameDisplay;
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

    private Timeline pulseTimeline;

    /**
     * Initialize: set up pulse animation for waiting state.
     */
    @FXML
    public void initialize() {
        // Pulse animation for the waiting label
        pulseTimeline = new Timeline();
        pulseTimeline.setCycleCount(Timeline.INDEFINITE);
    }

    /**
     * Handle PLAY button click.
     * Validates username, shows shake if empty, otherwise transitions to waiting
     * state.
     */
    @FXML
    private void handlePlayClicked(ActionEvent event) {
        // gRPC-safe: wrapping for future streaming thread compatibility
        Platform.runLater(() -> {
            String username = usernameField.getText().trim();

            if (username.isEmpty()) {
                // Shake animation on the username field
                TranslateTransition shake = new TranslateTransition(Duration.millis(50), usernameField);
                shake.setCycleCount(4);
                shake.setAutoReverse(true);
                shake.setByX(8);
                shake.play();
                return;
            }

            // Store username in SessionData
            SessionData.username = username;

            // Transition to waiting state
            entryContainer.setVisible(false);
            entryContainer.setManaged(false);
            waitingContainer.setVisible(true);
            waitingContainer.setManaged(true);

            // Update waiting label with username
            usernameDisplay.setText("Playing as: " + username);

            // Start pulse animation
            startPulseAnimation();
        });
    }

    /**
     * Handle emoji button click with scale animation.
     */
    @FXML
    private void handleEmojiClicked(ActionEvent event) {
        // gRPC-safe: wrapping for future streaming thread compatibility
        Platform.runLater(() -> {
            Button clickedButton = (Button) event.getSource();
            String emoji = (String) clickedButton.getUserData();

            // Scale animation
            ScaleTransition scaleUp = new ScaleTransition(Duration.millis(75), clickedButton);
            scaleUp.setToX(1.3);
            scaleUp.setToY(1.3);

            ScaleTransition scaleDown = new ScaleTransition(Duration.millis(75), clickedButton);
            scaleDown.setToX(1.0);
            scaleDown.setToY(1.0);

            scaleUp.setOnFinished(e -> scaleDown.play());
            scaleUp.play();

            System.out.println("Reaction sent: " + emoji);
        });
    }

    /**
     * Start the pulsing animation on the waiting label.
     */
    private void startPulseAnimation() {
        // gRPC-safe: wrapping for future streaming thread compatibility
        Platform.runLater(() -> {
            pulseTimeline.getKeyFrames().clear();

            // Toggle opacity: 1.0 to 0.5 to 1.0
            Duration half = Duration.millis(400);
            Duration full = Duration.millis(800);

            javafx.animation.KeyValue kv1 = new javafx.animation.KeyValue(
                    waitingLabel.opacityProperty(), 1.0);
            javafx.animation.KeyValue kv2 = new javafx.animation.KeyValue(
                    waitingLabel.opacityProperty(), 0.5);
            javafx.animation.KeyValue kv3 = new javafx.animation.KeyValue(
                    waitingLabel.opacityProperty(), 1.0);

            javafx.animation.KeyFrame kf1 = new javafx.animation.KeyFrame(Duration.ZERO, kv1);
            javafx.animation.KeyFrame kf2 = new javafx.animation.KeyFrame(half, kv2);
            javafx.animation.KeyFrame kf3 = new javafx.animation.KeyFrame(full, kv3);

            pulseTimeline.getKeyFrames().addAll(kf1, kf2, kf3);
            pulseTimeline.play();
        });
    }
}
