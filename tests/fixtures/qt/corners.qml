// QT-03c §5 amendment #2 fixture: a 200×200 parent with a small
// Rectangle pinned to each corner. Exercises the four corner
// combinations.

import QtQuick 2.15

Item {
    id: root
    width: 200
    height: 200

    // Top-left: anchors.left + anchors.top.
    Rectangle {
        id: tlBadge
        width: 30
        height: 20
        anchors.left: parent.left
        anchors.top: parent.top
        color: "#ff0000"
    }

    // Top-right: anchors.right + anchors.top.
    Rectangle {
        id: trBadge
        width: 30
        height: 20
        anchors.right: parent.right
        anchors.top: parent.top
        color: "#00ff00"
    }

    // Bottom-left: anchors.left + anchors.bottom.
    Rectangle {
        id: blBadge
        width: 30
        height: 20
        anchors.left: parent.left
        anchors.bottom: parent.bottom
        color: "#0000ff"
    }

    // Bottom-right: anchors.right + anchors.bottom.
    Rectangle {
        id: brBadge
        width: 30
        height: 20
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        color: "#ffff00"
    }
}
