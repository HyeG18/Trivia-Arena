import threading
import redis

from PyQt6.QtCore import QThread, pyqtSignal

from networking.grpc_client import ModeratorGrpcClient
import config


class GrpcStreamWorker(QThread):
    """
    Background QThread that owns the PlayStream connection.

    Signals (always emitted to the main/GUI thread via Qt's queued connection):
      leaderboard_updated(players: list[dict], total_responses: int)
      players_connected_changed(count: int)
      question_launched(text: str, options: list[str], time_limit: int)
      error_occurred(message: str)
    """

    leaderboard_updated = pyqtSignal(list, int)
    players_connected_changed = pyqtSignal(int)
    question_launched = pyqtSignal(str, list, int)
    error_occurred = pyqtSignal(str)

    def __init__(self, parent=None):
        super().__init__(parent)
        self._stop_event = threading.Event()
        self._client = ModeratorGrpcClient()
        self._redis_client = redis.Redis(host=config.REDIS_HOST, port=config.REDIS_PORT, decode_responses=True)
        self._last_player_count = -1

    # ------------------------------------------------------------------ #
    # QThread entry point
    # ------------------------------------------------------------------ #

    def run(self):
        try:
            # Start a separate thread to monitor player count
            player_monitor_thread = threading.Thread(
                target=self._monitor_player_count,
                daemon=True
            )
            player_monitor_thread.start()

            responses = self._client.open_play_stream(self._stop_event)
            for msg in responses:
                if self._stop_event.is_set():
                    break

                if msg.HasField("leaderboard"):
                    lb = msg.leaderboard
                    players = [
                        {
                            "rank": p.rank,
                            "username": p.username,
                            "score": p.score,
                            "last_correct": p.last_answer_correct,
                        }
                        for p in lb.top_players
                    ]
                    # Qt queued connection ensures this runs on the GUI thread.
                    self.leaderboard_updated.emit(players, lb.total_responses)

                elif msg.HasField("new_question"):
                    q = msg.new_question
                    self.question_launched.emit(q.text, list(q.options), q.time_limit_sec)

        except Exception as exc:
            if not self._stop_event.is_set():
                self.error_occurred.emit(str(exc))

    # ------------------------------------------------------------------ #
    # Player count monitoring (runs in background)
    # ------------------------------------------------------------------ #

    def _monitor_player_count(self):
        """Poll Redis every second to get the count of connected players."""
        while not self._stop_event.is_set():
            try:
                # Count active sessions: pattern "session:*"
                player_count = self._redis_client.dbsize()  # Total keys

                # More precise: count only session keys
                session_keys = self._redis_client.keys("session:*")
                player_count = len(session_keys) if session_keys else 0

                # Only emit if count changed
                if player_count != self._last_player_count:
                    self._last_player_count = player_count
                    self.players_connected_changed.emit(player_count)

            except Exception:
                pass  # Silent fail on Redis connection issues

            # Check every 500ms
            self._stop_event.wait(0.5)

    # ------------------------------------------------------------------ #
    # Shutdown
    # ------------------------------------------------------------------ #

    def stop(self):
        self._stop_event.set()
        self._client.close()
        self.wait(3000)  # ms
