import QtQuick 2.12

Item {
    width: 720
    height: 480

    Rectangle {
        id: header
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        height: 60
    }

    Rectangle {
        id: art
        anchors.top: header.bottom
        anchors.right: parent.right
        width: 160
        height: 160
    }

    Text {
        id: title
        anchors.left: parent.left
        anchors.right: art.left
        anchors.top: art.top
        anchors.bottom: art.bottom
    }

    Rectangle {
        id: footer
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: 48
    }
}
