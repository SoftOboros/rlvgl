// QT-03c §5 amendment fixture: a 200×200 parent with four
// children, each anchored to one parent edge. Exercises every
// single-edge anchor lowering path.

import QtQuick 2.15

Item {
    id: root
    width: 200
    height: 200

    // anchors.left: child.x = parent.x; height literal.
    Rectangle {
        id: leftBar
        height: 30
        anchors.left: parent.left
        color: "#ff0000"
    }

    // anchors.right: requires literal width; right edge.
    Rectangle {
        id: rightBar
        width: 40
        height: 30
        anchors.right: parent.right
        color: "#00ff00"
    }

    // anchors.top: child.y = parent.y; width literal.
    Rectangle {
        id: topBar
        width: 50
        anchors.top: parent.top
        color: "#0000ff"
    }

    // anchors.bottom: requires literal height; bottom edge.
    Rectangle {
        id: bottomBar
        width: 60
        height: 35
        anchors.bottom: parent.bottom
        color: "#ffff00"
    }
}
