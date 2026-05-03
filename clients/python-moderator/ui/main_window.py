from PyQt6.QtWidgets import QMainWindow, QWidget, QHBoxLayout, QStackedWidget

from ui.sidebar import Sidebar
from ui.views.question_bank_view import QuestionBankView
from ui.views.live_room_view import LiveRoomView
from workers.grpc_worker import GrpcStreamWorker


class MainWindow(QMainWindow):
    """
    Root window.

    ┌──────────┬────────────────────────────────┐
    │ Sidebar  │  QStackedWidget                │
    │  (220px) │   index 0 → QuestionBankView   │
    │          │   index 1 → LiveRoomView        │
    └──────────┴────────────────────────────────┘

    The GrpcStreamWorker QThread is owned here so its lifetime is tied to the
    window, not to any individual view.
    """

    def __init__(self):
        super().__init__()
        self.setWindowTitle("Arena de Preguntas — Moderator Dashboard")
        self.setMinimumSize(1200, 720)
        self.resize(1440, 860)

        # ── Central widget ────────────────────────────────
        central = QWidget()
        central.setObjectName("centralWidget")
        self.setCentralWidget(central)

        root = QHBoxLayout(central)
        root.setContentsMargins(0, 0, 0, 0)
        root.setSpacing(0)

        # ── Sidebar ───────────────────────────────────────
        self._sidebar = Sidebar()
        root.addWidget(self._sidebar)

        # ── Stacked views ─────────────────────────────────
        self._stack = QStackedWidget()
        self._stack.setObjectName("contentArea")
        root.addWidget(self._stack, stretch=1)

        self._question_bank = QuestionBankView()
        self._live_room = LiveRoomView(
            question_bank_getter=self._question_bank.get_questions
        )
        self._stack.addWidget(self._question_bank)   # index 0
        self._stack.addWidget(self._live_room)        # index 1

        self._sidebar.view_changed.connect(self._stack.setCurrentIndex)

        # ── gRPC stream worker ────────────────────────────
        # Runs on a background QThread; emits signals safely back to GUI thread.
        self._stream_worker = GrpcStreamWorker(self)
        self._stream_worker.leaderboard_updated.connect(
            self._live_room.on_leaderboard_updated
        )
        self._stream_worker.players_connected_changed.connect(
            self._live_room.on_players_connected_changed
        )
        self._stream_worker.question_launched.connect(
            self._live_room.on_question_launched
        )
        self._stream_worker.error_occurred.connect(self._on_stream_error)
        self._stream_worker.start()

    # ── Error forwarding ──────────────────────────────────

    def _on_stream_error(self, msg: str) -> None:
        self._sidebar.set_stream_status(connected=False)
        self._live_room.on_stream_error(msg)

    # ── Lifecycle ─────────────────────────────────────────

    def closeEvent(self, event):
        self._stream_worker.stop()
        self._live_room.cleanup()
        super().closeEvent(event)
