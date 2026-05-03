#!/usr/bin/env python3
"""
Arena de Preguntas — Moderator Dashboard
Entry point.

Before the first run:
    pip install -r requirements.txt
    python generate_stubs.py      # compile .proto → grpc_generated/

Then:
    python main.py
"""
import sys

from PyQt6.QtWidgets import QApplication
from PyQt6.QtGui import QFont

from ui.main_window import MainWindow
from ui.styles import STYLESHEET


def main() -> None:
    app = QApplication(sys.argv)
    app.setApplicationName("Arena Moderator")
    app.setStyleSheet(STYLESHEET)
    app.setFont(QFont("Segoe UI", 10))

    window = MainWindow()
    window.show()

    sys.exit(app.exec())


if __name__ == "__main__":
    main()
