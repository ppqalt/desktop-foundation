"""Select the current viewport with Kitty's native selection machinery."""
from kittens.tui.handler import result_handler


def main(args):
    pass


@result_handler(no_ui=True)
def handle_result(args, result, target_window_id, boss):
    window = boss.window_id_map.get(target_window_id)
    if window is None:
        return
    screen = window.screen
    screen.start_selection(0, 0)
    screen.update_selection(screen.columns - 1, screen.lines - 1, False, True)
