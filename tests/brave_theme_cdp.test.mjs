import test from 'node:test';
import assert from 'node:assert/strict';
import {endpoint, request, installed, verified, reload} from '../scripts/brave_theme_cdp.mjs';

test('discovery connects only to localhost browser endpoint', () => {
  assert.equal(endpoint('9222\n/devtools/browser/abc-123\n'), 'ws://127.0.0.1:9222/devtools/browser/abc-123');
  for (const input of ['0\n/devtools/browser/a','70000\n/devtools/browser/a','9222\nws://example.com','9222\n/devtools/page/a','9222\n/devtools/browser/a\nextra']) {
    assert.throws(() => endpoint(input));
  }
});
test('CDP methods are strictly lifecycle allowlisted', () => {
  for (const method of ['Target.getTargets','Runtime.evaluate','Network.getAllCookies','Browser.close','Page.navigate']) assert.throws(() => request(1,method));
  assert.equal(JSON.parse(request(1,'Extensions.loadUnpacked',{path:'/theme'})).method,'Extensions.loadUnpacked');
});
test('only already installed enabled theme at exact path is accepted', () => {
  const good={id:'same',name:'Desktop Foundation Graphite',enabled:true,path:'/theme',version:'1'};
  assert.equal(installed({extensions:[good]},'/theme').id,'same');
  for(const entries of [[],[good,good],[{...good,enabled:false}],[{...good,path:'/different'}],[{...good,name:'Other'}]]) assert.throws(()=>installed({extensions:entries},'/theme'));
});
test('reload must preserve ID and apply exact manifest version', () => {
  const before={id:'same'};
  verified(before,{id:'same'},{id:'same',version:'2'},'2');
  assert.throws(()=>verified(before,{id:'other'},{id:'same',version:'2'},'2'));
  assert.throws(()=>verified(before,{id:'same'},{id:'same',version:'1'},'2'));
});
test('short-lived connection uses only lifecycle calls and closes', async () => {
  const original=globalThis.WebSocket; const calls=[]; let socket;
  class Fake extends EventTarget {
    constructor(){super();socket=this;this.closed=false;this.version='1';queueMicrotask(()=>this.dispatchEvent(new Event('open')))}
    send(text){const r=JSON.parse(text);calls.push(r.method);if(r.method==='Extensions.loadUnpacked')this.version='2';const result=r.method==='Extensions.loadUnpacked'?{id:'same'}:{extensions:[{id:'same',name:'Desktop Foundation Graphite',path:'/theme',enabled:true,version:this.version}]};queueMicrotask(()=>this.dispatchEvent(new MessageEvent('message',{data:JSON.stringify({id:r.id,result})})))}
    close(){this.closed=true}
  }
  globalThis.WebSocket=Fake;
  try{await reload('ws://127.0.0.1:9222/devtools/browser/a','/theme','2');assert.equal(socket.closed,true);assert.deepEqual(calls,['Extensions.getExtensions','Extensions.loadUnpacked','Extensions.getExtensions']);await assert.rejects(reload('ws://127.0.0.1:9222/devtools/browser/a','/theme','3'),/verification failed/);assert.equal(socket.closed,true)}
  finally{globalThis.WebSocket=original}
});
