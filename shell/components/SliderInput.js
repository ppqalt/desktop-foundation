.pragma library

// Horizontal adjustment is shared by single and range sliders. Keep vertical
// movement for focus navigation. Modified combinations never adjust values.
function direction(event, mirrored) {
    if (event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier))
        return 0;
    const key = event.key;
    const left = key === Qt.Key_Left || key === Qt.Key_A;
    const right = key === Qt.Key_Right || key === Qt.Key_D;
    return (left ? -1 : right ? 1 : 0) * (mirrored ? -1 : 1);
}

// The same key contract for every slider and each range handle. Owners provide
// their allowed bounds; range constraints never need a second keyboard policy.
function adjust(event, node, minimum, maximum, mirrored) {
    if (event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier))
        return false;
    const movement = direction(event, mirrored);
    if (event.key === Qt.Key_Home)
        node.value = minimum;
    else if (event.key === Qt.Key_End)
        node.value = maximum;
    else if (movement > 0)
        node.increase();
    else if (movement < 0)
        node.decrease();
    else
        return false;
    node.value = Math.max(minimum, Math.min(maximum, node.value));
    return true;
}
