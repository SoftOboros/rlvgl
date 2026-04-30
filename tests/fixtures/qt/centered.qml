// QT-03c fixture: a Rectangle with literal width+height anchored
// `centerIn: parent` inside a sized Item. Exercises the QT-03c
// centered-bounds lowering path. Within a 200×200 parent, a 50×50
// child should land at (75, 75, 50, 50).

import QtQuick 2.15

Item {
    id: root
    width: 200
    height: 200

    Rectangle {
        id: badge
        width: 50
        height: 50
        anchors.centerIn: parent
        color: "#ff8800"
    }
}
