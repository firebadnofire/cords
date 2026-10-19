import QtQuick

// Original stroke icons drawn by Qt; no platform glyph/font or remote asset dependency.
Canvas {
    id: icon
    property string name: ""
    property color color: Theme.secondary
    implicitWidth: 20
    implicitHeight: 20
    onNameChanged: requestPaint()
    onColorChanged: requestPaint()
    onWidthChanged: requestPaint()
    onHeightChanged: requestPaint()
    onAvailableChanged: if (available) requestPaint()
    onVisibleChanged: if (visible) requestPaint()
    onPaint: {
        const ctx = getContext("2d");
        ctx.reset();
        ctx.scale(width / 24, height / 24);
        ctx.strokeStyle = color;
        ctx.lineWidth = 1.8;
        ctx.lineCap = "round";
        ctx.lineJoin = "round";
        ctx.beginPath();
        switch (name) {
        case "check": ctx.moveTo(5, 12); ctx.lineTo(10, 17); ctx.lineTo(19, 7); break;
        case "plus": ctx.moveTo(12, 5); ctx.lineTo(12, 19); ctx.moveTo(5, 12); ctx.lineTo(19, 12); break;
        case "close": ctx.moveTo(6, 6); ctx.lineTo(18, 18); ctx.moveTo(18, 6); ctx.lineTo(6, 18); break;
        case "more": for (const x of [5, 12, 19]) { ctx.moveTo(x + 1, 12); ctx.arc(x, 12, 1, 0, 2 * Math.PI); } break;
        case "chevron": ctx.moveTo(6, 9); ctx.lineTo(12, 15); ctx.lineTo(18, 9); break;
        case "search": ctx.arc(10, 10, 6, 0, 2 * Math.PI); ctx.moveTo(15, 15); ctx.lineTo(20, 20); break;
        case "refresh": ctx.arc(12, 12, 8, 0.5, 5.5); ctx.moveTo(18, 3); ctx.lineTo(18, 8); ctx.lineTo(13, 8); break;
        case "send": ctx.moveTo(3, 3); ctx.lineTo(21, 12); ctx.lineTo(3, 21); ctx.lineTo(6, 12); ctx.closePath(); ctx.moveTo(6, 12); ctx.lineTo(14, 12); break;
        case "info": ctx.arc(12, 12, 9, 0, 2 * Math.PI); ctx.moveTo(12, 11); ctx.lineTo(12, 17); ctx.moveTo(12, 7); ctx.lineTo(12, 7.3); break;
        case "users": ctx.arc(10, 8, 3.5, 0, 2 * Math.PI); ctx.moveTo(3, 21); ctx.lineTo(3, 18); ctx.bezierCurveTo(3, 11, 17, 11, 17, 18); ctx.lineTo(17, 21); ctx.moveTo(17, 5); ctx.bezierCurveTo(23, 5, 23, 12, 18, 12); ctx.moveTo(20, 15); ctx.lineTo(21, 20); break;
        case "lock": ctx.rect(5, 10, 14, 11); ctx.moveTo(8, 10); ctx.lineTo(8, 7); ctx.bezierCurveTo(8, 1, 16, 1, 16, 7); ctx.lineTo(16, 10); break;
        case "switch": ctx.moveTo(3, 8); ctx.lineTo(21, 8); ctx.lineTo(17, 4); ctx.moveTo(21, 16); ctx.lineTo(3, 16); ctx.lineTo(7, 20); break;
        case "copy": ctx.rect(8, 8, 12, 13); ctx.moveTo(15, 5); ctx.lineTo(15, 3); ctx.lineTo(3, 3); ctx.lineTo(3, 16); ctx.lineTo(5, 16); break;
        case "settings":
            for (let i = 0; i < 16; ++i) {
                const angle = Math.PI * i / 8;
                const radius = i % 2 ? 7 : 10;
                const x = 12 + radius * Math.cos(angle), y = 12 + radius * Math.sin(angle);
                if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
            }
            ctx.closePath(); ctx.moveTo(15, 12); ctx.arc(12, 12, 3, 0, 2 * Math.PI); break;
        }
        ctx.stroke();
    }
}
