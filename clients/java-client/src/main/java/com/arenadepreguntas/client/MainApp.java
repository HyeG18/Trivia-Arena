package com.arenadepreguntas.client;

import javafx.application.Application;
import javafx.fxml.FXMLLoader;
import javafx.scene.Scene;
import javafx.scene.image.Image;
import javafx.stage.Stage;

import java.io.IOException;

/**
 * Main entry point for Arena de Preguntas Java Player Client.
 * Loads the lobby scene and manages the primary stage.
 */
public class MainApp extends Application {

    @Override
    public void start(Stage primaryStage) throws IOException {
        // Load lobby FXML
        FXMLLoader fxmlLoader = new FXMLLoader(
                getClass().getResource("/com/arenadepreguntas/client/fxml/lobby.fxml"));
        javafx.scene.Parent root = fxmlLoader.load();
        Scene lobbyScene = new Scene(root, 1280, 720);

        // Load and apply CSS stylesheet
        String css = getClass().getResource("/com/arenadepreguntas/client/css/arena_style.css")
                .toExternalForm();
        lobbyScene.getStylesheets().add(css);

        // Configure primary stage
        primaryStage.setTitle("Arena de Preguntas — Player Client");
        primaryStage.setScene(lobbyScene);
        primaryStage.setWidth(1280);
        primaryStage.setHeight(720);
        primaryStage.setResizable(true);

        // Optional: Set window icon (if available)
        try {
            primaryStage.getIcons().add(
                    new Image(getClass().getResourceAsStream("/com/arenadepreguntas/client/icon.png")));
        } catch (Exception e) {
            // Icon not found, continue without it
        }

        primaryStage.setOnCloseRequest(event -> GrpcClientService.shutdownIfInitialized());
        primaryStage.show();
    }

    /**
     * Launch the JavaFX application.
     */
    public static void main(String[] args) {
        launch(args);
    }
}
