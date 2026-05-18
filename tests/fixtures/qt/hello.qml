// Minimal QML smoke fixture for rlvgl-creator's `qt ingest`.
// Exercises imports, properties, signals, handlers, grouped properties,
// dotted assignment targets, child items, and an object-valued assignment.

import QtQuick 2.15
import QtQuick.Controls as QC

Item {
    id: root
    width: 800
    height: 480

    property string title: "Hello"
    property int count: 0
    readonly property real ratio: 1.5

    signal pressed(int x, int y)

    anchors.fill: parent
    anchors.margins: 16

    font {
        pixelSize: 32
        family: "Inter"
    }

    Rectangle {
        id: bg
        anchors.fill: parent
        color: "#1e1e2e"
    }

    QC.Label {
        text: root.title
        anchors.centerIn: parent
    }

    MouseArea {
        anchors.fill: parent
        onClicked: root.count += 1
        onPressed: { console.log("down"); root.pressed(0, 0) }
    }

    transitions: Transition { from: "*"; to: "active" }
}
