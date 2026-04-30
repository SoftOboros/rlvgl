// QT-07 fixture: a screen mixing `Image { source: … }` references
// (qrc: prefix and relative paths) with font declarations across
// the three forms QT-07 §5 recognises:
//   1. `Text { font.family: "<name>" }`
//   2. dotted `<*>.font.family: "<name>"`
//   3. standalone `Font { family: "<name>" }` blocks (deferred
//      visual-only example; we use the dotted form below to keep
//      the fixture parseable by the QT-01a structural parser).

import QtQuick 2.15
import QtQuick.Controls 2.15

Item {
    id: root
    width: 320
    height: 200

    Image {
        id: bg
        source: "qrc:/icons/background.png"
    }

    Image {
        id: play
        source: "icons/play.png"
    }

    Image {
        id: stop
        source: "qrc:///icons/stop.png"
    }

    // Dedup: the same path declared on a second Image must not
    // produce a duplicate entry in the inventory.
    Image {
        id: bg_alt
        source: "qrc:/icons/background.png"
    }

    Text {
        id: heading
        font.family: "FiraSans Bold"
        text: "Title"
    }

    Text {
        id: body
        font.family: "Roboto"
        text: "Body copy"
    }

    Text {
        id: tagline
        font.family: "Roboto"
        text: "Dedup target"
    }
}
