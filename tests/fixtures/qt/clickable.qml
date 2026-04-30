// QT-04 fixture: a Button + onClicked exercises the signal-handler
// lowering path. Kept tight so the golden is reviewable.

import QtQuick.Controls 2.15

Button {
    text: "Press me"
    width: 200
    height: 80
    onClicked: count += 1
}
