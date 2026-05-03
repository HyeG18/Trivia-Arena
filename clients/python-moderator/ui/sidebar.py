from PyQt6.QtWidgets import QFrame, QVBoxLayout, QLabel, QPushButton
from PyQt6.QtCore import pyqtSignal


class Sidebar(QFrame):
    """Left navigation panel. Emits view_changed(int) when the user switches tabs."""

    view_changed = pyqtSignal(int)  # 0 = Question Bank, 1 = Live Room

    _NAV_ITEMS = [
        ("📋  Question Bank", 0),
        ("🎮  Live Room",     1),
    ]

    def __init__(self, parent=None):
        super().__init__(parent)
        self.setObjectName("sidebar")
        self.setFixedWidth(220)

        layout = QVBoxLayout(self)
        layout.setContentsMargins(16, 28, 16, 24)
        layout.setSpacing(4)

        # ── Brand ────────────────────────────────────────
        title = QLabel("ARENA")
        title.setObjectName("appTitle")
        subtitle = QLabel("MODERATOR DASHBOARD")
        subtitle.setObjectName("appSubtitle")
        layout.addWidget(title)
        layout.addWidget(subtitle)

        layout.addSpacing(36)

        nav_section = QLabel("NAVIGATION")
        nav_section.setObjectName("sectionTitle")
        layout.addWidget(nav_section)
        layout.addSpacing(6)

        # ── Nav buttons ──────────────────────────────────
        self._buttons: list[QPushButton] = []
        for label, index in self._NAV_ITEMS:
            btn = QPushButton(label)
            btn.setObjectName("navBtn")
            btn.setCheckable(True)
            btn.setAutoExclusive(True)
            btn.clicked.connect(lambda _, i=index: self.view_changed.emit(i))
            layout.addWidget(btn)
            self._buttons.append(btn)

        layout.addStretch()

        # ── Connection status ─────────────────────────────
        layout.addWidget(self._make_divider())
        layout.addSpacing(10)

        self._dot = QLabel("● CONNECTED")
        self._dot.setObjectName("statusDot")
        self._dot.setStyleSheet("color: #00C853; font-size: 10px;")

        self._host_lbl = QLabel("api-gateway · :8080")
        self._host_lbl.setObjectName("statusLabel")

        layout.addWidget(self._dot)
        layout.addWidget(self._host_lbl)

        # Select first tab by default
        self._buttons[0].setChecked(True)

    # ── Public API ───────────────────────────────────────

    def set_stream_status(self, connected: bool) -> None:
        if connected:
            self._dot.setText("● CONNECTED")
            self._dot.setStyleSheet("color: #00C853; font-size: 10px;")
        else:
            self._dot.setText("● DISCONNECTED")
            self._dot.setStyleSheet("color: #FF5252; font-size: 10px;")

    # ── Helpers ──────────────────────────────────────────

    @staticmethod
    def _make_divider() -> QFrame:
        d = QFrame()
        d.setObjectName("divider")
        d.setFrameShape(QFrame.Shape.HLine)
        return d
