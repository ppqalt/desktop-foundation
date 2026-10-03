import json, socket, threading, subprocess, time, os, tempfile, shutil
from pathlib import Path
R=Path(__file__).resolve().parent.parent; adapters=R/'shell/adapters/niri'
import unittest


class NiriAdapter(unittest.TestCase):
    @unittest.skipUnless(os.environ.get("DF_TEST_NIRI_IPC") == "1" and shutil.which("quickshell") and os.environ.get("WAYLAND_DISPLAY"), "opt-in live Qt runtime: DF_TEST_NIRI_IPC=1")
    def test_event_stream_and_reconnect(self):
        with tempfile.TemporaryDirectory(prefix='foundation-niri-ipc-') as td:
         P=Path(td);path=P/'ipc.sock'; streams=[]; seen=[]; generation=[1]; stall_outputs=[True]; stop=threading.Event()
         server=socket.socket(socket.AF_UNIX);server.bind(str(path));server.listen();server.settimeout(.1)
         window=lambda i:{'id':i,'title':'Owned IPC fixture','app_id':'fixture','workspace_id':70,'is_focused':True,'is_floating':False,'is_urgent':False,'layout':{'tile_pos_in_workspace_view':None,'window_size':[900,1000]},'focus_timestamp':None}
         workspace=lambda idx:{'id':70,'idx':idx,'name':None,'output':'fixture-output','is_active':True,'is_focused':True,'is_urgent':False,'active_window_id':generation[0]}
         def send(conn,value):conn.sendall((json.dumps(value)+'\n').encode())
         def client(conn):
          try:
           value=json.loads(conn.makefile().readline());seen.append(value)
           if value=='EventStream':
            streams.append(conn);send(conn,{'Ok':'Handled'});send(conn,{'WorkspacesChanged':{'workspaces':[workspace(1)]}});send(conn,{'WindowsChanged':{'windows':[window(generation[0])]}});send(conn,{'KeyboardLayoutsChanged':{'keyboard_layouts':{'names':['Finnish'],'current_idx':0}}})
            while not stop.is_set() and conn.fileno()>=0:time.sleep(.03)
           elif value=='Outputs':
            if stall_outputs[0]:stall_outputs[0]=False;stop.wait(5)
            else:send(conn,{'Ok':{'Outputs':{'fixture-output':{'name':'fixture-output','make':'fixture','model':'output','logical':{'x':0,'y':0,'width':1920,'height':1080,'scale':1}}}}})
           elif value=={'Action':{'FocusWindow':{'id':777}}}:stop.wait(5)
           else:send(conn,{'Ok':'Handled'})
          except (OSError,ValueError):pass
          finally:
           if conn not in streams:conn.close()
         def accept():
          while not stop.is_set():
           try:conn,_=server.accept();threading.Thread(target=client,args=(conn,),daemon=True).start()
           except socket.timeout:pass
           except OSError:break
         threading.Thread(target=accept,daemon=True).start();(P/'adapter').symlink_to(adapters,target_is_directory=True)
         (P/'shell.qml').write_text('''import Quickshell
        import Quickshell.Io
        import "adapter"
        ShellRoot {
         id: fixture
         property int timeouts: 0
         Adapter { id: adapter; onLastErrorChanged: {if (lastError.includes("timed out")) fixture.timeouts++;} }
         IpcHandler { target: "fixture"
          function status(): string { return JSON.stringify({ready:adapter.ready, windows:adapter.windows, workspaces:adapter.workspaces, focused:adapter.focusedWindow, error:adapter.lastError, timeouts:fixture.timeouts}); }
          function focus(id:string):void {adapter.focusWindow(id);}
          function screenshot():void {adapter.screenshotWindow();}
         }
        }''')
         log=open(P/'adapter.log','w');proc=subprocess.Popen(['quickshell','--path',str(P)],env={**os.environ,'NIRI_SOCKET':str(path),'XDG_CACHE_HOME':str(P/'cache'),'XDG_STATE_HOME':str(P/'state')},stdout=log,stderr=log)
         def ipc(method,*args):
          result=subprocess.run(['quickshell','ipc','--pid',str(proc.pid),'call','fixture',method,*args],capture_output=True,text=True,timeout=5)
          return result
         def state():
          result=ipc('status')
          try:return json.loads(result.stdout)
          except ValueError:return None
         def wait(predicate):
          end=time.monotonic()+8
          while time.monotonic()<end:
           s=state()
           if s and predicate(s):return s
           time.sleep(.05)
          raise RuntimeError('Timed out: '+str(state()))
         try:
          wait(lambda s:s['ready'] and s['focused']['id']=='1' and s['timeouts']==1);c=streams[-1]
          time.sleep(.2)
          centers=lambda:len([x for x in seen if isinstance(x,dict) and 'CenterColumn' in x.get('Action',{})])
          initial_centers=centers();assert initial_centers==1
          send(c,{'WindowOpenedOrChanged':{'window':window(2)}});time.sleep(.25)
          assert centers()==initial_centers, 'Two windows in one column must not auto-center'
          send(c,{'WindowOpenedOrChanged':{'window':{**window(2),'is_floating':True}}});time.sleep(.25)
          assert centers()==initial_centers, 'Floating companions also prevent auto-centering'
          send(c,{'WindowClosed':{'id':2}});send(c,{'WindowFocusChanged':{'id':1}});time.sleep(.25)
          assert centers()==initial_centers+1, 'Closing to one must recenter'
          send(c,{'WindowLayoutsChanged':{'changes':[[1,window(1)['layout']]]}});time.sleep(.25)
          assert centers()==initial_centers+1, 'Layout events must not create a centering loop'
          send(c,{'FutureUnknownEvent':{'new_field':True}});send(c,{'WindowFocusChanged':{'id':None}});wait(lambda s:s['ready'] and s['focused'] is None)
          send(c,{'WorkspacesChanged':{'workspaces':[workspace(5)]}});s=wait(lambda s:s['workspaces'][0]['index']==5);assert s['workspaces'][0]['id']=='70'
          send(c,{'WorkspaceUrgencyChanged':{'id':70,'urgent':True}});wait(lambda s:s['workspaces'][0]['urgent'])
          send(c,{'WindowOpenedOrChanged':{'window':{**window(1),'workspace_id':999}}});s=wait(lambda s:s['windows'][0]['workspaceId']=='999');assert s['windows'][0]['outputId'] is None
          send(c,{'WindowClosed':{'id':1}});wait(lambda s:not s['windows'] and s['focused'] is None)
          ipc('focus','1');ipc('screenshot');time.sleep(.2)
          assert {'Action':{'FocusWindow':{'id':1}}} in seen
          shot=next(x['Action']['ScreenshotWindow'] for x in seen if isinstance(x,dict) and 'ScreenshotWindow' in x.get('Action',{}));assert shot['show_pointer'] is False and shot['write_to_disk'] is False
          shots=lambda:len([x for x in seen if isinstance(x,dict) and 'ScreenshotWindow' in x.get('Action',{})])
          before=shots();ipc('focus','777');ipc('screenshot')
          wait(lambda s:s['ready'] and s['timeouts']==2 and shots()==before+1)
          assert seen.count({'Action':{'FocusWindow':{'id':777}}})==1, 'Unknown-outcome actions must never be retried'
          c.shutdown(socket.SHUT_RDWR);c.close();wait(lambda s:not s['ready'] and not s['windows']);generation[0]=2;wait(lambda s:s['ready'] and s['focused']['id']=='2');assert len(streams)>=2
          print('PASS: event snapshots, centering, unknown/null/cross-resource events, action routing, stalled-output recovery, stalled-action queue recovery without replay and disconnect/reconnect')
         finally:
          stop.set();proc.terminate();proc.wait(timeout=5);server.close()
          for c in streams:
           try:c.close()
           except OSError:pass
          log.close()
