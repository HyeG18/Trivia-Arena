package com.arenadepreguntas.client.controller;

import javafx.animation.TranslateTransition;
import javafx.application.Platform;
import javafx.event.ActionEvent;
import javafx.fxml.FXML;
import javafx.scene.control.Button;
import javafx.scene.control.Label;
import javafx.scene.layout.StackPane;
import javafx.scene.layout.VBox;
import javafx.util.Duration;

public class LeaderboardController {
    @FXML
    private StackPane leaderboardOverlay;
    @FXML
    private VBox leaderboardCard;
    @FXML
    private Label selfLeaderboardUsername;
    @FXML
    private Label selfLeaderboardScore;
    @FXML
    private Button nextQuestionButton;

    private ArenaController arenaController;

    /**
     * Initialize: store reference to arena controller (will be set by
     * ArenaController).
     */
    @FXML
    public void initialize() {
        // Inject from ArenaController via FXMLLoader
    }

    /**
     * Set arena controller reference for callback.
     */
    public void setArenaController(ArenaController controller) {
        // gRPC-safe: wrapping for future streaming thread compatibility
        Platform.runLater(() -> {
            this.arenaController = controller;
        });
    }

    /**
     * Handle NEXT QUESTION button click: slide down overlay, trigger next round.
     */
    @FXML
    private void handleNextQuestion(ActionEvent event) {
        // gRPC-safe: wrapping for future streaming thread compatibility
        Platform.runLater(() -> {
            if (leaderboardCard != null) {
                // Slide down animation
                TranslateTransition slideDown = new TranslateTransition(Duration.millis(200), leaderboardCard);
                slideDown.setToY(600);
                slideDown.setOnFinished(e -> {
                    Platform.runLater(() -> {
                        if (leaderboardOverlay != null) {
                            leaderboardOverlay.setVisible(false);
                            leaderboardOverlay.setManaged(false);
                        }

                        // Trigger next mock question if arena controller available
                        if (arenaController != null) {
                            arenaController.startMockRound();
                        }
                    });
                });
                slideDown.play();
            }
        });
    }

    /**
     * Populate leaderboard with player scores.
     * gRPC-safe: must be called via Platform.runLater() in production.
     */
    public void populate(String[] usernames, int[] scores) {
        // gRPC-safe: wrapping for future streaming thread compatibility
        Platform.runLater(() -> {
            // Mock data is already in FXML, but this method is provided for future updates
            if (usernames != null && scores != null) {
                for (int i = 0; i < Math.min(4, usernames.length); i++) {
                    // Update first 4 rows dynamically if needed
                    // For now, mock is hardcoded in FXML
                }
            }
        });
    }
}
