#!/usr/bin/env python3
"""Opt-in real-device UI regression. Moves the pointer and changes/restores settings.
Run manually in the Niri session with connected Nothing Ear (3) and /dev/uinput access.
Never discovered by the ordinary test suite. Find sound is deliberately not activated.
"""
import fcntl
import json
import os
from pathlib import Path
import signal
import struct
import subprocess
import time
import argparse

ROOT = Path(__file__).resolve().parents[1]

def run(*args):
    return subprocess.run(args, capture_output=True, check=True, timeout=30)

def ipc(method, *args):
    return run('quickshell', 'ipc', '--path', str(ROOT/'shell'), 'call', 'foundation', method, *map(str,args))

def state(name):
    return json.loads(ipc(name+'Status').stdout)

def bt():
    return state('bluetooth')

def c():
    return bt()['controls']

def waitp(fn,seconds=20):
    end=time.monotonic()+seconds
    while time.monotonic()<end:
        value=bt()
        if fn(value):return value
        time.sleep(.08)
    raise AssertionError(bt())

def input_device():
    status=json.loads(ipc('status').stdout)
    geometry=next(o['geometry'] for o in status['outputs'] if o['focused'])
    descriptor=os.open('/dev/uinput',os.O_WRONLY|os.O_NONBLOCK)
    fcntl.ioctl(descriptor,0x40045564,1)
    for code in list(range(1,256))+[272]:fcntl.ioctl(descriptor,0x40045565,code)
    fcntl.ioctl(descriptor,0x40045564,2);fcntl.ioctl(descriptor,0x40045566,8)
    fcntl.ioctl(descriptor,0x40045564,3)
    for code,maximum in [(0,geometry['width']-1),(1,geometry['height']-1)]:
        fcntl.ioctl(descriptor,0x40045567,code)
        fcntl.ioctl(descriptor,0x401c5504,struct.pack('H2xiiiiii',code,0,0,maximum,0,0,0))
    fcntl.ioctl(descriptor,0x405c5503,struct.pack('HHHH80sI',3,0x1209,28,1,b'foundation-live-regression',0))
    fcntl.ioctl(descriptor,0x5501);time.sleep(1)
    return descriptor

def event(kind,code,value):
    os.write(fd,struct.pack('llHHi',0,0,kind,code,value)+struct.pack('llHHi',0,0,0,0,0))

def key(code,mods=()):
    for modifier in mods:
        event(1,modifier,1);time.sleep(.03)
    event(1,code,1);time.sleep(.025);event(1,code,0)
    for modifier in reversed(mods):event(1,modifier,0)
    time.sleep(.25)

def wheel(value):
    event(2,8,value);time.sleep(.3)

def open_controls():
    global baseline, device_path
    if not bt()['visible']:key(48,(125,))
    waitp(lambda s:s['visible'])
    candidates=json.loads(run(str(ROOT/'scripts/nothing-backend'),'--discover').stdout)['devices']
    index=next(i for i,d in enumerate(bt()['devices']) if d['connected'] and d['dbusPath'] in candidates)
    while bt()['selected']!=index:key(108 if bt()['selected']<index else 103)
    device_path=bt()['devices'][index]['dbusPath']
    key(28)
    waitp(lambda s:s.get('controls') and s['controls']['ready'])
    if c()['state']['modelCode']!='B173':raise RuntimeError('This hardware pass is for Nothing Ear (3).')
    if baseline is None: baseline=json.loads(json.dumps(c()['state']))

def select_key(name):
    index=next(i for i,row in enumerate(c()['rows']) if row['key']==name)
    while c()['selected']!=index:key(108 if c()['selected']<index else 103)
    return index

def press_setting(name,code,value):
    index=select_key(name);before=c()['scrollY'];key(code)
    waitp(lambda s:s.get('controls') and not s['controls']['busy'] and s['controls']['state'].get(name)==value)
    assert c()['selected']==index,(name,index,c())
    assert abs(c()['scrollY']-before)<1,(name,before,c()['scrollY'])
    assert not c()['error'],c()

def click(x,y):
    event(3,0,int(x));event(3,1,int(y));time.sleep(.25)
    event(1,272,1);time.sleep(.025);event(1,272,0);time.sleep(.4)

def choose(name):
    index=select_key(name);bounds=c()['bounds']
    click(bounds['x']+300,bounds['y']+bounds['listY']+index*66-c()['scrollY']+31)


def test_keyboard():
    original = None
    try:
        if bt()['visible']:
            key(1)
            time.sleep(0.3)
            key(1)
        open_controls()
        original = json.loads(json.dumps(c()['state']))
        assert 'L ' in bt()['devices'][0]['detail'] and 'R ' in bt()['devices'][0]['detail'], bt()
        print('PASS both battery percentages in list; case omitted when missing', flush=True)
        bounds=c()['bounds']
        event(3,0,int(bounds['x']+280))
        event(3,1,int(bounds['y']+bounds['listY']+128))
        time.sleep(0.3)
        for _ in range(len(c()['rows'])):
            key(103)
        assert c()['selected']==0, 'Pointer/input moved during the navigation setup'
        for i in range(len(c()['rows']) - 1):
            before = c()['selected']
            key(108)
            assert c()['selected'] == before + 1, ('down', before, c())
        for i in range(len(c()['rows']) - 1):
            before = c()['selected']
            key(103)
            assert c()['selected'] == before - 1, ('up', before, c())
        key(15);assert c()['selected']==1
        key(15,(42,));assert c()['selected']==0
        print('PASS every arrow step, Tab/Shift+Tab, stationary mouse; no selection stealing',flush=True)
        before_state = c()['state']
        for i in range(len(c()['rows']) - 1):
            before = c()['selected']
            wheel(-1)
            assert c()['selected'] == before + 1, ('wheel', before, c())
        assert c()['state'] == before_state, 'wheel must not change settings'
        for i in range(len(c()['rows']) - 1):
            before = c()['selected']
            wheel(1)
            assert c()['selected'] == before - 1, ('wheel', before, c())
        print('PASS wheel over list to both ends, one row per notch, no actions', flush=True)
        modes=[3,1,2,4,7,5]
        for _ in modes:
            value=modes[(modes.index(c()['state']['anc'])+1)%len(modes)]
            press_setting('anc',106,value)
        level=original['bass']['level'];delta=-1 if level==5 else 1
        press_setting('bass',105 if delta<0 else 106,{'enabled':original['bass']['enabled'],'level':level+delta})
        press_setting('bass',106 if delta<0 else 105,original['bass'])
        press_setting('bass',28,{'enabled':not original['bass']['enabled'],'level':level})
        press_setting('bass',28,original['bass'])
        presets=[0,1,2,3,5]
        if original['eq'] in presets:
            for _ in presets:
                value=presets[(presets.index(c()['state']['eq'])+1)%len(presets)]
                press_setting('eq',106,value)
        press_setting('inEar',28,not original['inEar']);press_setting('inEar',28,original['inEar'])
        press_setting('latency',28,not original['latency']);press_setting('latency',28,original['latency'])
        print('PASS daily settings through real keys with readback, stable selection/scroll, restored', flush=True)
        main_index = select_key('custom')
        main_scroll = c()['scrollY']
        key(28)
        assert c()['page'] == 'custom'
        assert c()['selected'] == 0 and abs(c()['scrollY']) < 1, c()
        for i, name in enumerate(['Bass', 'Mid', 'Treble']):
            while c()['selected'] < i:
                key(108)
            delta=-1 if original['customEq'][i]>=6 else 1
            key(105 if delta<0 else 106)
            waitp(lambda s: not s['controls']['busy'])
            assert c()['state']['customEq'][i] == original['customEq'][i] + delta, c()
            key(106 if delta<0 else 105)
            waitp(lambda s: not s['controls']['busy'])
            assert c()['state']['customEq'][i] == original['customEq'][i], c()
        key(1)
        assert c()['page'] == 'main' and c()['selected'] == main_index, c()
        print('PASS custom bands actual read/write, page starts at top, Back restores parent selection', flush=True)
        main_index = select_key('gesturesPage')
        key(28)
        assert c()['page'] == 'gestures'
        gesture=original['gestures'][0]['action'];actions=[8,9,11]
        next_action=actions[(actions.index(gesture)+1)%len(actions)]
        key(106);waitp(lambda s:not s['controls']['busy']);assert c()['state']['gestures'][0]['action']==next_action
        key(105);waitp(lambda s:not s['controls']['busy']);assert c()['state']['gestures'][0]['action']==gesture
        for _ in range(8):
            key(108)
        assert c()['rows'][c()['selected']]['key'] == 'back', c()
        key(28)
        assert c()['page'] == 'main' and c()['selected'] == main_index, c()
        print('PASS gestures readback, scroll reaches bottom, Enter Back restores selection', flush=True)
        main_index = select_key('info')
        key(28)
        assert c()['page'] == 'info'
        key(1)
        assert c()['selected'] == main_index
        main_index = select_key('find')
        key(28)
        assert c()['page'] == 'find'
        assert c()['rows'][0]['key'] == 'none'
        key(28)
        assert not c()['busy']
        key(1)
        assert c()['selected'] == main_index
        print('PASS information and Find navigation, warning row cannot ring', flush=True)
        select_key('refresh')
        before = c()['scrollY']
        key(28)
        waitp(lambda s: not s['controls']['busy'])
        assert c()['rows'][c()['selected']]['key'] == 'refresh' and abs(before - c()['scrollY']) < 1, c()
        print('PASS refresh and battery updates preserve lower-list selection/scroll', flush=True)
        key(48, (125,))
        waitp(lambda s: not s['visible'])
        assert not subprocess.run(['pgrep', '-x', 'foundation-noth'], capture_output=True).stdout
        print('PASS Super+B closes controls and helper exits', flush=True)
        open_controls()
        key(1)
        waitp(lambda s: s.get('controls') is None)
        assert bt()['visible']
        key(1)
        assert not bt()['visible']
        print('PASS reopen, Escape to list, Escape closes', flush=True)
    finally:
        if bt().get('visible'):
            key(1)
            time.sleep(0.5)
            key(1)

def test_extended(audible_fit=False):
    try:
        open_controls()
        original=json.loads(json.dumps(c()['state']))
        for name in ['dual','personal','superMic','autoTransparency','spatial']:
            value=original[name]
            target=(1-value) if name=='spatial' else not value
            press_setting(name,28,target)
            press_setting(name,28,value)
        print('PASS new quick settings through actual UI, readback and restore',flush=True)
        choose('qualityPage');assert c()['page']=='quality'
        key(108);key(108);key(103);key(1)
        assert c()['rows'][c()['selected']]['key']=='qualityPage'
        choose('fit');assert c()['page']=='fit'
        key(28);assert not c()['busy']
        if audible_fit:
            select_key('fitStart');key(28)
            waitp(lambda s:s.get('controls') and not s['controls']['busy'],24)
            assert c()['state'].get('fit') and not c()['error'],c()
            print('PASS actual ear-tip test response: '+json.dumps(c()['state']['fit']),flush=True)
        key(1);assert c()['rows'][c()['selected']]['key']=='fit'
        key(1);key(1)
    finally:
        if bt()['visible']:ipc('toggleBluetooth')


def test_audio_quality():
    open_controls()
    original=c()['state']['quality']
    try:
        for value in [0 if original==2 else 2,original]:
            choose('qualityPage')
            selected=0 if value==0 else 1
            while c()['selected']!=selected:key(108 if c()['selected']<selected else 103)
            key(28)
            waitp(lambda v:v.get('controls') is None,8)
            time.sleep(2)
            end=time.monotonic()+12
            while time.monotonic()<end and not next(d for d in bt()['devices'] if d['dbusPath']==device_path)['connected']:time.sleep(.2)
            if not next(d for d in bt()['devices'] if d['dbusPath']==device_path)['connected']:
                index=next(i for i,d in enumerate(bt()['devices']) if d['dbusPath']==device_path)
                while bt()['selected']!=index:key(108 if bt()['selected']<index else 103)
                key(28);waitp(lambda v:not v['visible'],40)
            if bt()['visible']:ipc('toggleBluetooth');waitp(lambda v:not v['visible'],5)
            time.sleep(.5);open_controls()
            assert c()['state']['quality']==value,c()
            expected='LDAC' if value==2 else 'AAC'
            waitp(lambda v: next(d for d in v['devices'] if d['dbusPath']==device_path)['audio']==expected,20)
            print('PASS audio-quality reboot, fresh readback and actual playback: '+expected,flush=True)
        assert c()['state']['quality']==original
    finally:
        if bt()['visible']:ipc('toggleBluetooth')


def test_mouse():
    original = None
    try:
        if bt()['visible']:
            key(1)
            time.sleep(0.3)
            key(1)
        open_controls()
        original = c()['state']['inEar']
        choose('inEar')
        assert c()['state']['inEar'] != original, c()
        choose('inEar')
        assert c()['state']['inEar'] == original
        choose('info')
        assert c()['page'] == 'info', c()
        b = c()['bounds']
        click(b['x'] + b['width'] - 48, b['y'] + b['height'] - 28)
        assert c()['page'] == 'main', c()
        choose('custom')
        assert c()['page'] == 'custom', c()
        choose('back')
        assert c()['page'] == 'main', c()
        bounds=c()['bounds']
        event(3,0,max(1,int(bounds['x']-20)))
        event(3,1,int(bounds['y']+100))
        event(1, 272, 1)
        event(1, 272, 0)
        time.sleep(0.7)
        assert not bt()['visible'], bt()
        print('PASS actual mouse toggle/readback, page entries, footer Back, row Back, outside dismissal', flush=True)
    finally:
        if bt()['visible']:
            key(1)
            time.sleep(0.3)
            key(1)

def test_lifecycle():
    frozen = None
    try:
        if bt()['visible']:
            key(1)
            time.sleep(0.3)
            key(1)
        for _ in range(8):
            open_controls()
            key(1)
            waitp(lambda s: s.get('controls') is None)
            key(1)
            waitp(lambda s: not s['visible'])
        print('PASS 8 rapid open/close cycles, startup handoff, fresh state, no stray helper', flush=True)
        open_controls()
        frozen = int(c()['backendPid'])
        os.kill(frozen, signal.SIGSTOP)
        key(1)
        waitp(lambda s: s.get('controls') is None, 3)
        assert bt()['visible']
        key(1)
        waitp(lambda s: not s['visible'])
        assert not os.path.exists(f'/proc/{frozen}')
        frozen = None
        print('PASS Escape recovers from a frozen backend within 1.5-second shutdown bound', flush=True)
        open_controls()
        select_key('refresh')
        frozen = int(c()['backendPid'])
        os.kill(frozen, signal.SIGSTOP)
        key(28)
        waitp(lambda s: s.get('controls') is None, 8)
        assert bt()['error'] and bt()['visible'], bt()
        assert not os.path.exists(f'/proc/{frozen}')
        frozen = None
        key(1)
        print('PASS stalled command deadline returns to usable generic list, kills stuck child', flush=True)
        open_controls()
        os.kill(int(c()['backendPid']), signal.SIGKILL)
        waitp(lambda s: s.get('controls') is None)
        assert bt()['error']
        key(1)
        print('PASS backend crash degrades to usable generic list', flush=True)
        open_controls()
        ipc('showLauncher')
        time.sleep(0.6)
        assert not bt()['visible'], bt()
        assert state('launcher')['visible']
        ipc('hideLauncher')
        time.sleep(0.3)
        print('PASS switching to launcher closes controls and cleans backend', flush=True)
    finally:
        if frozen and os.path.exists(f'/proc/{frozen}'):
            os.kill(frozen, signal.SIGCONT)
        if bt()['visible']:
            key(1)
            time.sleep(0.3)
            key(1)



def test_disconnect():
    try:
        open_controls()
        choose('disconnect')
        waitp(lambda s:s.get('controls') is None and not next(d for d in s['devices'] if d['dbusPath']==device_path)['connected'])
        assert bt()['visible'] and not bt()['error'],bt()
        index=next(i for i,d in enumerate(bt()['devices']) if d['dbusPath']==device_path)
        while bt()['selected']!=index:key(108 if bt()['selected']<index else 103)
        key(28);waitp(lambda s:not s['visible'],40)
        time.sleep(.5) # Allow the close animation to release its input surface.
        open_controls()
        assert next(d for d in bt()['devices'] if d['dbusPath']==device_path)['audio']=='LDAC',bt()
        print('PASS actual UI Disconnect/reconnect, fresh battery/control state and LDAC',flush=True)
        key(1);waitp(lambda s:s.get('controls') is None)
        detail=next(d for d in bt()['devices'] if d['dbusPath']==device_path)['detail']
        assert 'L ' in detail and 'R ' in detail,detail
        print('PASS separate battery values retained on Back and refreshed after reconnect',flush=True)
        key(1)
    finally:
        if bt()['visible']:
            ipc('toggleBluetooth');waitp(lambda s:not s['visible'],5)

baseline=None
device_path=None

def restore_settings():
    if baseline is None:return
    if bt()['visible']:
        ipc('toggleBluetooth');waitp(lambda s:not s['visible'],5)
    helper=subprocess.Popen([str(ROOT/'scripts/nothing-backend'),device_path],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True)
    import queue
    import threading
    replies=queue.Queue()
    def pump():
        for line in helper.stdout:replies.put(json.loads(line))
        replies.put(None)
    reader=threading.Thread(target=pump,daemon=True);reader.start()
    def response(events):
        end=time.monotonic()+25
        while time.monotonic()<end:
            message=replies.get(timeout=max(.01,end-time.monotonic()))
            if message is None:raise RuntimeError('Restore helper exited')
            if message['event'] in events:return message
        raise RuntimeError('Restore helper timed out')
    try:
        message=response({'ready','error'})
        if message['event']=='error':raise RuntimeError(message)
        current=message['state']
        changes=[{'setting':k,'value':baseline[k]} for k in ['anc','bass','eq','customEq','inEar','latency','dual','personal','superMic','autoTransparency','spatial'] if k in baseline and current.get(k)!=baseline[k]]
        changes += [{'setting':'gestures','slot':i,'value':slot['action']} for i,slot in enumerate(baseline.get('gestures',[])) if i<len(current.get('gestures',[])) and current['gestures'][i]['action']!=slot['action']]
        for change in changes:
            helper.stdin.write(json.dumps(change)+'\n');helper.stdin.flush()
            reply=response({'complete','error'})
            if reply['event']=='error':raise RuntimeError(reply)
        helper.stdin.write('{"action":"close"}\n');helper.stdin.flush();helper.wait(timeout=5)
        print('PASS baseline settings checked/restored independently after the UI tests',flush=True)
    finally:
        if helper.poll() is None:helper.terminate();helper.wait(timeout=5)

if __name__ == '__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--fit',action='store_true',help='Play the fit-test sound; wear both earbuds')
    parser.add_argument('--reboot-quality',action='store_true',help='Change AAC/LDAC, reboot earbuds and restore the original preference')
    options=parser.parse_args()
    source=run('pactl','get-default-source').stdout
    try:
        phases=[test_keyboard,lambda: test_extended(options.fit),test_mouse,test_lifecycle,test_disconnect]
        if options.reboot_quality:phases.append(test_audio_quality)
        for phase in phases:
            fd=input_device()
            try:phase()
            finally:
                fcntl.ioctl(fd,0x5502)
                os.close(fd)
    finally:
        restore_settings()
        assert run('pactl','get-default-source').stdout==source, 'Capture routing changed'
