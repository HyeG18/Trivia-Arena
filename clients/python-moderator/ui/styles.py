STYLESHEET = """
/* ============================================================
   Arena de Preguntas — Moderator Dashboard
   Dark Cyber-Minimalist Theme
   ============================================================ */

* {
    font-family: 'Segoe UI', 'Inter', 'Roboto', sans-serif;
    color: #E8EAF0;
    box-sizing: border-box;
}

QMainWindow, QWidget#centralWidget {
    background-color: #0B0F19;
}

/* ── Sidebar ──────────────────────────────────────────────── */

QFrame#sidebar {
    background-color: #0D1117;
    border-right: 1px solid rgba(255, 255, 255, 0.06);
}

QPushButton#navBtn {
    background-color: transparent;
    border: none;
    border-radius: 8px;
    padding: 12px 16px;
    text-align: left;
    font-size: 13px;
    color: #6B7280;
}
QPushButton#navBtn:hover  { background-color: rgba(157, 78, 221, 0.12); color: #C792EA; }
QPushButton#navBtn:checked {
    background-color: rgba(157, 78, 221, 0.18);
    color: #9D4EDD;
    border-left: 3px solid #9D4EDD;
    border-radius: 0px 8px 8px 0px;
}

/* ── Cards / Surfaces ─────────────────────────────────────── */

QFrame#card {
    background-color: rgba(255, 255, 255, 0.035);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 12px;
}

QFrame#kpiCard {
    background-color: rgba(0, 210, 255, 0.05);
    border: 1px solid rgba(0, 210, 255, 0.18);
    border-radius: 10px;
}

/* ── Typography ───────────────────────────────────────────── */

QLabel#appTitle    { font-size: 17px; font-weight: bold; color: #9D4EDD; }
QLabel#appSubtitle { font-size: 9px;  color: #4B5563; letter-spacing: 1px; }

QLabel#viewTitle    { font-size: 22px; font-weight: bold; color: #E8EAF0; }
QLabel#sectionTitle { font-size: 10px; font-weight: bold; color: #6B7280;  letter-spacing: 1.2px; }

QLabel#kpiValue { font-size: 38px; font-weight: bold; color: #00D2FF; }
QLabel#kpiLabel { font-size: 10px; color: #6B7280; letter-spacing: 0.8px; }

QLabel#activeQuestion { font-size: 17px; font-weight: bold; color: #E8EAF0; line-height: 1.5; }

QLabel#statusDot   { font-size: 10px; }
QLabel#statusLabel { font-size: 11px; color: #6B7280; }

/* ── Buttons ──────────────────────────────────────────────── */

QPushButton#btnLaunch {
    background-color: #9D4EDD;
    color: #FFFFFF;
    border: none;
    border-radius: 12px;
    font-size: 15px;
    font-weight: bold;
    padding: 18px;
    min-height: 58px;
}
QPushButton#btnLaunch:hover    { background-color: #AE60EE; }
QPushButton#btnLaunch:pressed  { background-color: #7A3BB5; }
QPushButton#btnLaunch:disabled { background-color: #2D1F47; color: #4B5563; }

QPushButton#btnDanger {
    background-color: rgba(255, 82, 82, 0.1);
    color: #FF5252;
    border: 1px solid rgba(255, 82, 82, 0.35);
    border-radius: 8px;
    font-size: 12px;
    font-weight: bold;
    padding: 8px 16px;
}
QPushButton#btnDanger:hover   { background-color: rgba(255, 82, 82, 0.22); }
QPushButton#btnDanger:pressed { background-color: rgba(255, 82, 82, 0.4);  }

QPushButton#btnSecondary {
    background-color: rgba(255, 255, 255, 0.05);
    color: #9CA3AF;
    border: 1px solid rgba(255, 255, 255, 0.09);
    border-radius: 7px;
    font-size: 12px;
    padding: 7px 14px;
}
QPushButton#btnSecondary:hover { background-color: rgba(255, 255, 255, 0.09); }

QPushButton#btnSuccess {
    background-color: rgba(0, 200, 83, 0.1);
    color: #00C853;
    border: 1px solid rgba(0, 200, 83, 0.28);
    border-radius: 8px;
    font-size: 12px;
    font-weight: bold;
    padding: 8px 16px;
}
QPushButton#btnSuccess:hover { background-color: rgba(0, 200, 83, 0.2); }

/* ── Table ────────────────────────────────────────────────── */

QTableWidget {
    background-color: transparent;
    border: none;
    gridline-color: rgba(255, 255, 255, 0.04);
    selection-background-color: rgba(157, 78, 221, 0.18);
    alternate-background-color: rgba(255, 255, 255, 0.02);
}
QTableWidget::item {
    padding: 8px 12px;
    color: #C0C4CC;
    border-bottom: 1px solid rgba(255, 255, 255, 0.04);
}
QTableWidget::item:selected { background-color: rgba(157, 78, 221, 0.14); color: #E8EAF0; }

QHeaderView::section {
    background-color: rgba(255, 255, 255, 0.03);
    color: #6B7280;
    font-size: 10px;
    font-weight: bold;
    letter-spacing: 0.6px;
    padding: 8px 12px;
    border: none;
    border-bottom: 1px solid rgba(255, 255, 255, 0.07);
}

/* ── Form inputs ──────────────────────────────────────────── */

QLineEdit, QTextEdit, QSpinBox {
    background-color: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    padding: 9px 13px;
    color: #E8EAF0;
    font-size: 13px;
}
QLineEdit:focus, QTextEdit:focus, QSpinBox:focus {
    border-color: #9D4EDD;
    background-color: rgba(157, 78, 221, 0.07);
}

QSpinBox::up-button, QSpinBox::down-button {
    background-color: rgba(255, 255, 255, 0.06);
    border: none;
    width: 18px;
}

QComboBox {
    background-color: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    padding: 8px 13px;
    color: #E8EAF0;
    font-size: 13px;
}
QComboBox:focus         { border-color: #9D4EDD; }
QComboBox::drop-down    { border: none; width: 22px; }
QComboBox QAbstractItemView {
    background-color: #161B22;
    border: 1px solid rgba(255, 255, 255, 0.1);
    selection-background-color: rgba(157, 78, 221, 0.28);
    color: #E8EAF0;
    padding: 4px;
}

QRadioButton {
    color: #9CA3AF;
    font-size: 13px;
    spacing: 8px;
}
QRadioButton::indicator {
    width: 15px; height: 15px;
    border-radius: 8px;
    border: 2px solid #4B5563;
    background: transparent;
}
QRadioButton::indicator:checked { border-color: #9D4EDD; background-color: #9D4EDD; }

/* ── Scrollbar ────────────────────────────────────────────── */

QScrollBar:vertical   { background: transparent; width: 5px; margin: 0; }
QScrollBar:horizontal { background: transparent; height: 5px; }
QScrollBar::handle:vertical, QScrollBar::handle:horizontal {
    background: rgba(255, 255, 255, 0.12);
    border-radius: 3px;
    min-height: 24px;
}
QScrollBar::handle:vertical:hover, QScrollBar::handle:horizontal:hover {
    background: rgba(157, 78, 221, 0.45);
}
QScrollBar::add-line, QScrollBar::sub-line { height: 0; width: 0; }

/* ── Leaderboard rows ─────────────────────────────────────── */

QFrame#lbRow1 {
    background-color: rgba(255, 215, 0, 0.07);
    border: 1px solid rgba(255, 215, 0, 0.18);
    border-radius: 8px;
}
QFrame#lbRow2 {
    background-color: rgba(192, 192, 192, 0.05);
    border: 1px solid rgba(192, 192, 192, 0.14);
    border-radius: 8px;
}
QFrame#lbRow3 {
    background-color: rgba(205, 127, 50, 0.05);
    border: 1px solid rgba(205, 127, 50, 0.14);
    border-radius: 8px;
}
QFrame#lbRowN {
    background-color: rgba(255, 255, 255, 0.025);
    border: 1px solid rgba(255, 255, 255, 0.055);
    border-radius: 8px;
}

/* ── Misc ─────────────────────────────────────────────────── */

QFrame#divider { background-color: rgba(255, 255, 255, 0.055); max-height: 1px; }

QMessageBox {
    background-color: #161B22;
}
QMessageBox QLabel { color: #E8EAF0; }
QMessageBox QPushButton {
    background-color: rgba(255, 255, 255, 0.07);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    padding: 6px 16px;
    color: #E8EAF0;
}
QMessageBox QPushButton:hover { background-color: rgba(157, 78, 221, 0.2); }
"""
