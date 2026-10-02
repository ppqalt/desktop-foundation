"""Create a real Kitty selection across its retained main screen and scrollback.

Selection endpoints store the scrollback position at which they were created.
Use native scroll navigation to anchor both ends, then restore the user's view.
No screen text extraction, clipboard writes, overlays or external processes.
"""
from kittens.tui.handler import result_handler


def main(args):
    pass


@result_handler(no_ui=True)
def handle_result(args, result, target_window_id, boss):
    window = boss.window_id_map.get(target_window_id)
    if window is None:
        return
    window.finish_scroll_animation()
    screen = window.screen
    original_scroll = screen.scrolled_by
    try:
        # On the alternate screen native scroll_home/end are no-ops: select
        # that screen, never expose unrelated main-screen contents to an editor.
        window.scroll_home()
        screen.start_selection(0, 0)
        window.scroll_end()
        screen.update_selection(screen.columns - 1, screen.lines - 1, False, True)
    finally:
        screen.scroll_to_absolute(original_scroll)
