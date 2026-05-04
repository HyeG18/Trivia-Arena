from typing import Callable

from PyQt6.QtWidgets import (
    QComboBox,
    QFrame,
    QHBoxLayout,
    QLabel,
    QMessageBox,
    QPushButton,
    QSizePolicy,
    QVBoxLayout,
    QWidget,
)
from PyQt6.QtCore import Qt, QTimer

from networking.grpc_client import ModeratorGrpcClient
import config

_RANK_OBJECT_NAME = {1: "lbRow1", 2: "lbRow2", 3: "lbRow3"}
_RANK_EMOJI       = {1: "🥇",     2: "🥈",     3: "🥉"}


class LiveRoomView(QWidget):
    """
    View 2 — Live Room (gRPC Control).

    Layout:
      ┌─ KPI row ──────────────────────────────────────┐
      │  Players connected   |   Responses received    │
      └────────────────────────────────────────────────┘
      ┌─ Active question card ──┬─ Live leaderboard ───┐
      │  <question text>        │  🥇 Player A  3,400  │
      │  [Force End Timer]      │  🥈 Player B  2,950  │
      └─────────────────────────┴──────────────────────┘
      ┌─ Launch section ───────────────────────────────┐
      │  [Combo: select question]   [⚡ LAUNCH]        │
      └────────────────────────────────────────────────┘

    gRPC calls (LaunchQuestion, ForceEndTimer) are made directly on this widget's
    methods because they are fast unary calls — the gateway responds in < 100 ms.
    Long-running work (stream listening) lives in GrpcStreamWorker (QThread).
    """

    def __init__(self, question_bank_getter: Callable[[], list[dict]], parent=None):
        super().__init__(parent)
        self._get_questions = question_bank_getter
        self._grpc = ModeratorGrpcClient()
        self._build_ui()

    # ------------------------------------------------------------------ #
    # UI construction
    # ------------------------------------------------------------------ #

    def _build_ui(self) -> None:
        root = QVBoxLayout(self)
        root.setContentsMargins(32, 24, 32, 24)
        root.setSpacing(20)

        root.addLayout(self._build_header())
        root.addLayout(self._build_kpi_row())
        root.addLayout(self._build_main_row(), stretch=1)
        root.addWidget(self._build_launch_card())

    def _build_header(self) -> QHBoxLayout:
        row = QHBoxLayout()
        title = QLabel("Live Room")
        title.setObjectName("viewTitle")
        row.addWidget(title)
        row.addStretch()
        self._stream_status_lbl = QLabel("● STREAM ACTIVE")
        self._stream_status_lbl.setStyleSheet(
            "color: #00C853; font-size: 12px; font-weight: bold;"
        )
        row.addWidget(self._stream_status_lbl)
        return row

    def _build_kpi_row(self) -> QHBoxLayout:
        row = QHBoxLayout()
        row.setSpacing(16)
        self._kpi_players   = _KpiCard("0", "PLAYERS CONNECTED")
        self._kpi_responses = _KpiCard("0", "RESPONSES RECEIVED")
        row.addWidget(self._kpi_players)
        row.addWidget(self._kpi_responses)
        row.addStretch()
        return row

    def _build_main_row(self) -> QHBoxLayout:
        row = QHBoxLayout()
        row.setSpacing(20)
        row.addWidget(self._build_question_card(), stretch=3)
        row.addWidget(self._build_leaderboard_card(), stretch=2)
        return row

    def _build_question_card(self) -> QFrame:
        card = QFrame()
        card.setObjectName("card")
        layout = QVBoxLayout(card)
        layout.setContentsMargins(20, 16, 20, 16)
        layout.setSpacing(12)

        # Card header
        ch = QHBoxLayout()
        section = QLabel("ACTIVE QUESTION")
        section.setObjectName("sectionTitle")
        ch.addWidget(section)
        ch.addStretch()
        self._btn_force = QPushButton("⏱  Force End Timer")
        self._btn_force.setObjectName("btnDanger")
        self._btn_force.clicked.connect(self._force_end_timer)
        ch.addWidget(self._btn_force)
        layout.addLayout(ch)

        divider = QFrame()
        divider.setObjectName("divider")
        divider.setFrameShape(QFrame.Shape.HLine)
        layout.addWidget(divider)

        self._active_q_label = QLabel("No question launched yet.")
        self._active_q_label.setObjectName("activeQuestion")
        self._active_q_label.setWordWrap(True)
        self._active_q_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self._active_q_label.setMinimumHeight(90)
        layout.addWidget(self._active_q_label, stretch=1)

        return card

    def _build_leaderboard_card(self) -> QFrame:
        card = QFrame()
        card.setObjectName("card")
        layout = QVBoxLayout(card)
        layout.setContentsMargins(20, 16, 20, 16)
        layout.setSpacing(10)

        title = QLabel("LIVE LEADERBOARD")
        title.setObjectName("sectionTitle")
        layout.addWidget(title)

        self._lb_container = QVBoxLayout()
        self._lb_container.setSpacing(6)
        layout.addLayout(self._lb_container)
        layout.addStretch()

        self._render_leaderboard([])   # show placeholder
        return card

    def _build_launch_card(self) -> QFrame:
        card = QFrame()
        card.setObjectName("card")
        layout = QVBoxLayout(card)
        layout.setContentsMargins(20, 14, 20, 14)
        layout.setSpacing(12)

        header = QLabel("LAUNCH NEXT QUESTION")
        header.setObjectName("sectionTitle")
        layout.addWidget(header)

        # Question selector row
        sel_row = QHBoxLayout()
        sel_row.setSpacing(8)
        sel_row.addWidget(QLabel("Select:"))
        self._q_combo = QComboBox()
        self._q_combo.setPlaceholderText("Choose a question from the bank…")
        self._q_combo.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Fixed)
        sel_row.addWidget(self._q_combo)
        btn_reload = QPushButton("↻")
        btn_reload.setObjectName("btnSecondary")
        btn_reload.setFixedWidth(34)
        btn_reload.setToolTip("Reload questions from the bank")
        btn_reload.clicked.connect(self._refresh_combo)
        sel_row.addWidget(btn_reload)
        layout.addLayout(sel_row)

        self._btn_launch = QPushButton("⚡  LAUNCH NEXT QUESTION")
        self._btn_launch.setObjectName("btnLaunch")
        self._btn_launch.clicked.connect(self._launch_question)
        layout.addWidget(self._btn_launch)

        self._refresh_combo()
        return card

    # ------------------------------------------------------------------ #
    # Leaderboard renderer (called on GUI thread via Qt signal)
    # ------------------------------------------------------------------ #

    def _render_leaderboard(self, players: list[dict]) -> None:
        # Clear previous rows
        while self._lb_container.count():
            item = self._lb_container.takeAt(0)
            if item.widget():
                item.widget().deleteLater()

        if not players:
            placeholder = QLabel("Waiting for data…")
            placeholder.setObjectName("statusLabel")
            placeholder.setAlignment(Qt.AlignmentFlag.AlignCenter)
            self._lb_container.addWidget(placeholder)
            return

        for p in players:
            rank     = p.get("rank", 0)
            obj_name = _RANK_OBJECT_NAME.get(rank, "lbRowN")
            emoji    = _RANK_EMOJI.get(rank, str(rank))

            row_frame = QFrame()
            row_frame.setObjectName(obj_name)
            row_layout = QHBoxLayout(row_frame)
            row_layout.setContentsMargins(12, 7, 12, 7)
            row_layout.setSpacing(10)

            rank_lbl = QLabel(emoji)
            rank_lbl.setFixedWidth(30)
            rank_lbl.setAlignment(Qt.AlignmentFlag.AlignCenter)

            name_lbl = QLabel(p.get("username", "—"))
            name_lbl.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Preferred)

            score_lbl = QLabel(f"{p.get('score', 0):,}")
            score_lbl.setStyleSheet("color: #00D2FF; font-weight: bold;")
            score_lbl.setAlignment(Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter)

            last_correct = p.get("last_correct")
            if last_correct is True:
                ind = QLabel("✓")
                ind.setStyleSheet("color: #00C853; font-weight: bold;")
            elif last_correct is False:
                ind = QLabel("✗")
                ind.setStyleSheet("color: #FF5252; font-weight: bold;")
            else:
                ind = QLabel("")
            ind.setFixedWidth(18)

            row_layout.addWidget(rank_lbl)
            row_layout.addWidget(name_lbl)
            row_layout.addWidget(score_lbl)
            row_layout.addWidget(ind)

            self._lb_container.addWidget(row_frame)

    # ------------------------------------------------------------------ #
    # Slots wired by MainWindow (called on GUI thread via Qt signal)
    # ------------------------------------------------------------------ #

    def on_leaderboard_updated(self, players: list, total_responses: int) -> None:
        self._kpi_responses.set_value(str(total_responses))
        self._render_leaderboard(players)

    def on_players_connected_changed(self, count: int) -> None:
        """Update the KPI card when player count changes."""
        self._kpi_players.set_value(str(count))

    def on_question_launched(self, text: str, options: list, time_limit: int) -> None:
        """Update the active question display when a question is launched."""
        self._active_q_label.setText(text)

    def on_stream_connected(self) -> None:
        self._stream_status_lbl.setText("● STREAM ACTIVE")
        self._stream_status_lbl.setStyleSheet(
            "color: #00C853; font-size: 12px; font-weight: bold;"
        )

    def on_stream_error(self, msg: str) -> None:
        self._stream_status_lbl.setText("● STREAM ERROR")
        self._stream_status_lbl.setStyleSheet(
            "color: #FF5252; font-size: 12px; font-weight: bold;"
        )

    def on_emoji_received(self, username: str, emoji_code: str) -> None:
        """Show a temporary emoji toast in the leaderboard area."""
        toast = QLabel(f"{emoji_code}  {username}")
        toast.setAlignment(Qt.AlignmentFlag.AlignCenter)
        toast.setStyleSheet(
            "color: #FFD700; font-size: 22px; font-weight: bold; "
            "background: rgba(255,255,255,0.06); border-radius: 8px; padding: 4px 10px;"
        )
        self._lb_container.insertWidget(0, toast)
        QTimer.singleShot(3000, toast.deleteLater)

    # ------------------------------------------------------------------ #
    # gRPC actions (fast unary calls — safe to call directly)
    # ------------------------------------------------------------------ #

    def _launch_question(self) -> None:
        idx = self._q_combo.currentIndex()
        questions = self._get_questions()
        if idx < 0 or idx >= len(questions):
            QMessageBox.warning(self, "No Question", "Select a question from the bank first.")
            return

        q = questions[idx]
        try:
            ack = self._grpc.launch_question(
                text=q.get("text", ""),
                options=q.get("options", []),
                time_limit_sec=q.get("time_limit_sec", config.DEFAULT_TIME_LIMIT_SEC),
                correct_answer_index=q.get("correct_option_index", 0),
            )
            if ack.success:
                self._active_q_label.setText(q.get("text", ""))
            else:
                QMessageBox.warning(self, "Launch Failed", "The server rejected the question.")
        except Exception as exc:
            QMessageBox.critical(self, "gRPC Error", str(exc))

    def _force_end_timer(self) -> None:
        try:
            self._grpc.force_end_timer()
        except Exception as exc:
            QMessageBox.critical(self, "gRPC Error", str(exc))

    def _refresh_combo(self) -> None:
        questions = self._get_questions()
        self._q_combo.clear()
        for q in questions:
            text = q.get("text", "")
            preview = text[:65] + "…" if len(text) > 65 else text
            self._q_combo.addItem(preview)

    # ------------------------------------------------------------------ #
    # Lifecycle
    # ------------------------------------------------------------------ #

    def cleanup(self) -> None:
        self._grpc.close()


# ────────────────────────────────────────────────────────────────────────
# Small helper widget
# ────────────────────────────────────────────────────────────────────────

class _KpiCard(QFrame):
    def __init__(self, value: str, label: str, parent=None):
        super().__init__(parent)
        self.setObjectName("kpiCard")
        self.setFixedWidth(188)

        layout = QVBoxLayout(self)
        layout.setContentsMargins(16, 14, 16, 14)
        layout.setSpacing(4)

        self._val = QLabel(value)
        self._val.setObjectName("kpiValue")
        self._val.setAlignment(Qt.AlignmentFlag.AlignCenter)

        lbl = QLabel(label)
        lbl.setObjectName("kpiLabel")
        lbl.setAlignment(Qt.AlignmentFlag.AlignCenter)

        layout.addWidget(self._val)
        layout.addWidget(lbl)

    def set_value(self, value: str) -> None:
        self._val.setText(value)
