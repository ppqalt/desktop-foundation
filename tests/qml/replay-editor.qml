import QtQuick
import QtQuick.Window
import QtQuick.Layouts
import QtTest
import Quickshell
import "components"

ShellRoot {
    Window {
        id: window
        visible: true
        width: 720
        height: 1100
        ReplayEditor {
            id: editor
            anchors.fill: parent
            backend: Quickshell.env("DF_EDITOR_BINARY")
            Component.onCompleted: run(["resume", "0123456789abcdef00000001"])
        }
        TestCase {
            id: checks
            name: "ReplayEditor"
            optional: true
            when: false
            function test_real_snapshot_and_range_selection() {
                tryVerify(() => editor.session !== null, 20000);
                compare(editor.session.audio_tracks, 2);
                compare(editor.session.thumbnails.length, 10);
                const range = findChild(editor, "replaySelection");
                const footage = findChild(editor, "replayFootage");
                verify(range !== null && footage !== null);
                verify(findChild(editor, "replayPlayhead") === null);
                range.first.value = 1.25;
                range.first.moved();
                compare(editor.startTime, 1.25);
                range.second.value = 5.75;
                range.second.moved();
                compare(editor.endTime, 5.75);
                const start = findChild(editor, "clipStart");
                const end = findChild(editor, "clipEnd");
                start.input.text = "0:02.500";
                start.input.editingFinished();
                compare(editor.startTime, 2.5);
                compare(editor.endTime, 5.75);
                end.input.text = "0:07.125";
                end.input.editingFinished();
                compare(editor.endTime, 7.125);
                compare(editor.startTime, 2.5);
                editor.scrub(3.1);
                tryVerify(() => editor.previewUrl.includes("preview-000186.jpg"), 10000);
                footage.forceActiveFocus();
                const before = editor.playhead;
                keyClick(Qt.Key_Right);
                verify(editor.playhead > before);
                compare(editor.startTime, 2.5);
                compare(editor.endTime, 7.125);
                mouseClick(footage, footage.width / 2, footage.height / 2);
                verify(Math.abs(editor.playhead - editor.session.duration / 2) < 0.05);
                compare(editor.startTime, 2.5);
                compare(editor.endTime, 7.125);
                editor.select(0, editor.session.duration);
                verify(Math.abs(footage.mapToItem(range, 0, 0).x - range.firstCenter) < 0.5);
                verify(Math.abs(footage.mapToItem(range, footage.width, 0).x - range.secondCenter) < 0.5);
                compare(editor.startTime, 0);
                compare(editor.endTime, editor.session.duration);
                editor.select(1.25, 5.75);
            }
            function test_save_finishes_without_draft_management() {
                verify(findChild(editor, "browseReplayDrafts") === null);
                verify(findChild(editor, "captureReplay") === null);
                editor.select(0, editor.session.duration);
                const saveButton = findChild(editor, "saveReplayClip");
                tryVerify(() => saveButton.enabled, 10000);
                saveButton.clicked();
                tryVerify(() => editor.savedPath.length > 0 && !editor.busy, 10000);
                compare(editor.error, "");
                compare(editor.session, null);
            }
            function cleanupTestCase() {
                const capture = Quickshell.env("DF_EDITOR_CAPTURE");
                if (capture)
                    window.contentItem.grabToImage(result => {
                        if (!result.saveToFile(capture))
                            console.error("EDITOR_CAPTURE_FAILED");
                        console.log("EDITOR_QML_OK");
                        Qt.quit();
                    });
                else {
                    console.log("EDITOR_QML_OK");
                    Qt.quit();
                }
            }
        }
        Timer {
            interval: 150
            running: true
            onTriggered: {
                checks.test_real_snapshot_and_range_selection();
                checks.test_save_finishes_without_draft_management();
                checks.cleanupTestCase();
            }
        }
    }
}
