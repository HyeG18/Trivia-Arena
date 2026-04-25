package com.arenadepreguntas.client.controller;

import javafx.animation.FadeTransition;
import javafx.animation.Timeline;
import javafx.application.Platform;
import javafx.fxml.FXML;
import javafx.scene.control.Button;
import javafx.scene.control.Label;
import javafx.scene.control.ProgressBar;
import javafx.scene.layout.StackPane;
import javafx.util.Duration;

import com.arenadepreguntas.client.SessionData;

public class ArenaController {
    @FXML
    private StackPane arenaRoot;
    @FXML
    private ProgressBar timerBar;
    @FXML
    private Label questionMeta;
    @FXML
    private Label questionText;
    @FXML
    private Button answerBtnA;
    @FXML
    private Button answerBtnB;
    @FXML
    private Button answerBtnC;
    @FXML
    private Button answerBtnD;
    @FXML
    private Label headerUsername;
    @FXML
    private Label scoreLabel;
    @FXML
    private Label rankBadge;
    @FXML
    private Label avatarInitial;
    @FXML
    private StackPane avatarCircle;

    @FXML
    private StackPane leaderboardOverlay;

    private Timeline timerTimeline;
    private double currentProgress = 1.0;
    private int currentQuestionNumber = 1;
    private final int TOTAL_QUESTIONS = 10;
    private final double TIMER_DURATION_SECONDS = 20.0;

    /**
     * Initialize: load the first mock question and set up UI.
     */
    @FXML
    public void initialize() {
        Platform.runLater(() -> {
            // Update header with session data
            headerUsername.setText(SessionData.username);
            avatarInitial.setText(SessionData.username.substring(0, 1).toUpperCase());
            scoreLabel.setText(String.format("%,d pts", SessionData.currentScore));
            rankBadge.setText("🥇 #" + SessionData.currentRank);

            // Start the first mock round
            startMockRound();
        });
    }

    /**
     * Handle Answer Button A click.
     */
    @FXML
    private void handleAnswerClickedA() {
        handleAnswerSelected('A');
    }

    /**
     * Handle Answer Button B click.
     */
    @FXML
    private void handleAnswerClickedB() {
        handleAnswerSelected('B');
    }

    /**
     * Handle Answer Button C click.
     */
    @FXML
    private void handleAnswerClickedC() {
        handleAnswerSelected('C');
    }

    /**
     * Handle Answer Button D click.
     */
    @FXML
    private void handleAnswerClickedD() {
        handleAnswerSelected('D');
    }

    /**
     * Show new question: updates labels, resets timer, re-enables buttons.
     * gRPC-safe: must be called via Platform.runLater() in production.
     */
    public void showNewQuestion(String text, String[] answerChoices) {
        // gRPC-safe: wrapping for future streaming thread compatibility
        Platform.runLater(() -> {
            // Update question meta and text
            questionMeta.setText("Question " + currentQuestionNumber + " of " + TOTAL_QUESTIONS);
            questionText.setText(text);

            // Update answer buttons
            if (answerChoices != null && answerChoices.length >= 4) {
                answerBtnA.setText(answerChoices[0]);
                answerBtnB.setText(answerChoices[1]);
                answerBtnC.setText(answerChoices[2]);
                answerBtnD.setText(answerChoices[3]);
            }

            // Reset visual state
            answerBtnA.getStyleClass().removeAll("answer-selected", "answer-dimmed");
            answerBtnB.getStyleClass().removeAll("answer-selected", "answer-dimmed");
            answerBtnC.getStyleClass().removeAll("answer-selected", "answer-dimmed");
            answerBtnD.getStyleClass().removeAll("answer-selected", "answer-dimmed");
            answerBtnA.setDisable(false);
            answerBtnB.setDisable(false);
            answerBtnC.setDisable(false);
            answerBtnD.setDisable(false);

            // Reset timer bar
            timerBar.getStyleClass().removeAll("timer-warning", "timer-danger");
            timerBar.setStyle("");
            currentProgress = 1.0;
            timerBar.setProgress(currentProgress);

            // Play fade transition on question card
            FadeTransition fade = new FadeTransition(Duration.millis(300), questionText.getParent());
            fade.setFromValue(0.7);
            fade.setToValue(1.0);
            fade.play();

            // Start timer countdown
            startTimerCountdown();
        });
    }

    /**
     * Update score and rank in header.
     * gRPC-safe: must be called via Platform.runLater() in production.
     */
    public void updateScore(int newScore, int newRank) {
        // gRPC-safe: wrapping for future streaming thread compatibility
        Platform.runLater(() -> {
            SessionData.currentScore = newScore;
            SessionData.currentRank = newRank;
            scoreLabel.setText(String.format("%,d pts", newScore));
            rankBadge.setText("🥇 #" + newRank);
        });
    }

    /**
     * Handle answer button selection: apply visual state, disable other buttons.
     * gRPC-safe: must be called via Platform.runLater() in production.
     */
    private void handleAnswerSelected(char letter) {
        // gRPC-safe: wrapping for future streaming thread compatibility
        Platform.runLater(() -> {
            Button selectedButton;
            Button otherBtn1, otherBtn2, otherBtn3;

            switch (letter) {
                case 'A':
                    selectedButton = answerBtnA;
                    otherBtn1 = answerBtnB;
                    otherBtn2 = answerBtnC;
                    otherBtn3 = answerBtnD;
                    break;
                case 'B':
                    selectedButton = answerBtnB;
                    otherBtn1 = answerBtnA;
                    otherBtn2 = answerBtnC;
                    otherBtn3 = answerBtnD;
                    break;
                case 'C':
                    selectedButton = answerBtnC;
                    otherBtn1 = answerBtnA;
                    otherBtn2 = answerBtnB;
                    otherBtn3 = answerBtnD;
                    break;
                case 'D':
                    selectedButton = answerBtnD;
                    otherBtn1 = answerBtnA;
                    otherBtn2 = answerBtnB;
                    otherBtn3 = answerBtnC;
                    break;
                default:
                    return;
            }

            // Add selected class to chosen button
            if (!selectedButton.getStyleClass().contains("btn-answer-selected")) {
                selectedButton.getStyleClass().add("btn-answer-selected");
            }

            // Dim other buttons
            otherBtn1.getStyleClass().add("btn-answer-dimmed");
            otherBtn2.getStyleClass().add("btn-answer-dimmed");
            otherBtn3.getStyleClass().add("btn-answer-dimmed");

            // Disable all buttons
            answerBtnA.setDisable(true);
            answerBtnB.setDisable(true);
            answerBtnC.setDisable(true);
            answerBtnD.setDisable(true);

            System.out.println("Answer selected: " + letter);

            // Mock: update score and show leaderboard after 2 seconds
            Timeline delay = new Timeline();
            delay.getKeyFrames().add(new javafx.animation.KeyFrame(Duration.millis(2000)));
            delay.setOnFinished(e -> {
                updateScore(SessionData.currentScore + 100, SessionData.currentRank);
                showLeaderboard();
            });
            delay.play();
        });
    }

    /**
     * Show leaderboard overlay with slide-up animation.
     * gRPC-safe: must be called via Platform.runLater() in production.
     */
    public void showLeaderboard() {
        // gRPC-safe: wrapping for future streaming thread compatibility
        Platform.runLater(() -> {
            if (leaderboardOverlay != null) {
                leaderboardOverlay.setVisible(true);
                leaderboardOverlay.setManaged(true);

                // Get the card and animate from bottom
                javafx.scene.Node card = leaderboardOverlay.lookup("#leaderboardCard");
                if (card != null) {
                    card.setTranslateY(600);
                    javafx.animation.TranslateTransition slideUp = new javafx.animation.TranslateTransition(
                            Duration.millis(400), card);
                    slideUp.setToY(0);
                    slideUp.setInterpolator(javafx.animation.Interpolator.EASE_OUT);
                    slideUp.play();
                }

                // Populate leaderboard (update self score)
                Label selfUsername = (Label) leaderboardOverlay.lookup("#selfLeaderboardUsername");
                Label selfScore = (Label) leaderboardOverlay.lookup("#selfLeaderboardScore");
                if (selfUsername != null) {
                    selfUsername.setText(SessionData.username);
                    selfScore.setText(String.format("%,d", SessionData.currentScore));
                }
            }
        });
    }

    /**
     * Start mock round: load first question and start timer.
     */
    public void startMockRound() {
        // gRPC-safe: wrapping for future streaming thread compatibility
        Platform.runLater(() -> {
            // Hide leaderboard if visible
            if (leaderboardOverlay != null) {
                leaderboardOverlay.setVisible(false);
                leaderboardOverlay.setManaged(false);
            }

            currentQuestionNumber++;
            if (currentQuestionNumber > TOTAL_QUESTIONS) {
                currentQuestionNumber = 1;
                SessionData.currentScore = 0;
                SessionData.currentRank = 1;
            }

            // Mock question data
            String question = "Which planet in our solar system has the most moons?";
            String[] answers = {
                    "🔴  A. Mars",
                    "🔵  B. Saturn",
                    "🟡  C. Jupiter",
                    "🟢  D. Neptune"
            };

            showNewQuestion(question, answers);
        });
    }

    /**
     * Start the countdown timer (20 seconds).
     */
    private void startTimerCountdown() {
        // gRPC-safe: wrapping for future streaming thread compatibility
        Platform.runLater(() -> {
            if (timerTimeline != null) {
                timerTimeline.stop();
            }

            timerTimeline = new Timeline();
            currentProgress = 1.0;
            final int ticksPerSecond = 5;
            final int totalTicks = (int) (TIMER_DURATION_SECONDS * ticksPerSecond);

            for (int i = 0; i <= totalTicks; i++) {
                final int tick = i;
                double progress = 1.0 - ((double) i / totalTicks);
                double duration = (i * 200.0); // 200ms per tick

                javafx.animation.KeyFrame kf = new javafx.animation.KeyFrame(
                        Duration.millis(duration),
                        event -> {
                            Platform.runLater(() -> {
                                timerBar.setProgress(progress);

                                // Change color based on time remaining
                                double percentRemaining = progress * 100;
                                if (percentRemaining <= 25) {
                                    timerBar.getStyleClass().removeAll("timer-warning");
                                    if (!timerBar.getStyleClass().contains("timer-danger")) {
                                        timerBar.getStyleClass().add("timer-danger");
                                    }
                                } else if (percentRemaining <= 50) {
                                    timerBar.getStyleClass().remove("timer-danger");
                                    if (!timerBar.getStyleClass().contains("timer-warning")) {
                                        timerBar.getStyleClass().add("timer-warning");
                                    }
                                } else {
                                    timerBar.getStyleClass().removeAll("timer-warning", "timer-danger");
                                }
                            });
                        });
                timerTimeline.getKeyFrames().add(kf);
            }

            timerTimeline.setOnFinished(e -> {
                Platform.runLater(this::showLeaderboard);
            });

            timerTimeline.play();
        });
    }
}
