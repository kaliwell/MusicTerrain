(function(){
  'use strict';
  const engine=typeof window.wallpaperRegisterAudioListener==='function';
  let config={size:64,shape:1,noise:2,seed:42,intensity:1,sensitivity:1,autogain:true,cavespeed:.12,zoom:1,meshfps:15,orbitspeed:8,renderscale:1,orbit:true,orthographic:false,caves:true,trees:true,water:true,showcontrols:false,background:[.67,.81,.94]};
  const normalizer=MTAudio.createNormalizer();
  let fps=30,enginePaused=false,hidden=!engine&&document.hidden,simulation=false,failed=false,ready=false,busy=false,revision=1,dirty=true;
  let audio=MTAudio.zero(),lastAudio=-Infinity,phase=0,clock=0,angle=.9,lastFrame=performance.now(),lastDraw=0,lastJob=-Infinity,lastJobClock=0,renderer=null,worker=null,held=null;
  const diagnostics={engine,frames:0,meshes:0,dropped:0,generationMs:0,vertices:0,carved:0,audio:MTAudio.zero(),input:MTAudio.zero(),audioCallbacks:0,paused:false,config};
  const $=id=>document.getElementById(id);
  function fail(error){failed=true;$('error').hidden=false;$('error').textContent='壁纸未能加载\n'+String(error);console.error(error);}
  function apply(properties){const next=MTAudio.clean(properties,config);if(JSON.stringify(next)!==JSON.stringify(config)){config=next;diagnostics.config=config;revision++;dirty=true;sync();}}
  // Register synchronously at body end: WE can deliver properties/audio before
  // worker initialization. Always retain only the latest audio observation.
  window.wallpaperPropertyListener={
    applyUserProperties:apply,
    applyGeneralProperties(properties){if(Number.isFinite(Number(properties.fps)))fps=Math.max(1,Math.min(240,Number(properties.fps)||30));},
    setPaused(value){enginePaused=Boolean(value);lastFrame=performance.now();lastDraw=0;diagnostics.paused=enginePaused||hidden;}
  };
  function receiveAudio(values){lastAudio=performance.now();diagnostics.input=MTAudio.spectrum(values);audio=normalizer.update(diagnostics.input,lastAudio/1000,config.sensitivity,config.autogain);diagnostics.audio=audio;diagnostics.audioCallbacks++;}
  if(engine)window.wallpaperRegisterAudioListener(receiveAudio);
  window.MusicTerrain={diagnostics,applyProperties:apply,receiveAudio,setPaused:window.wallpaperPropertyListener.setPaused};
  function sync(){
    document.body.classList.toggle('wallpaper',engine);document.body.classList.toggle('controls',config.showcontrols);
    const rgb=config.background.map(v=>Math.round(v*255));document.body.style.background=`radial-gradient(ellipse at 50% 25%,rgb(${rgb.map(v=>Math.min(255,v+22)).join(',')}) 0%,rgb(${rgb.join(',')}) 85%)`;
    for(const input of document.querySelectorAll('[data-property]')){const key=input.dataset.property;if(input.type==='checkbox')input.checked=config[key];else input.value=String(config[key]);}
    $('source').textContent=engine?'系统音频由 Wallpaper Engine 提供':'浏览器预览 · 系统音频在 Wallpaper Engine 中接入';
    $('demo').hidden=engine;
  }
  function accept(message){
    if(message.revision!==revision){diagnostics.dropped++;dirty=true;return;}
    renderer.upload(message);diagnostics.meshes++;diagnostics.generationMs=message.milliseconds;
    diagnostics.vertices=(message.solid.length+message.water.length)/7;diagnostics.carved=message.carved;
    $('status').textContent=engine?(diagnostics.audioCallbacks===0?'等待系统音频，请开启音频处理':`音频输入 ${diagnostics.input.rms.toFixed(4)} → 响应 ${diagnostics.audio.bass.toFixed(2)} · ${diagnostics.audioCallbacks} 次回调`):simulation?'模拟音乐预览 · 无声音':'浏览器预览 · 等待音频';
    $('metrics').textContent=`完整精度 · ${config.size} × ${config.size}\n生成 ${message.milliseconds.toFixed(1)} ms · ${(diagnostics.vertices/3).toLocaleString()} 个三角形`;
    if(engine)$('source').textContent=diagnostics.audioCallbacks===0?'尚未收到系统频谱，请开启壁纸的音频处理':diagnostics.input.rms<=1e-5?'音频接口已连接，当前没有声音':`已检测到系统音乐 · 输入 ${diagnostics.input.rms.toFixed(4)} / 响应 ${diagnostics.audio.rms.toFixed(2)}`;
  }
  function frame(now){
    requestAnimationFrame(frame);const dt=Math.min(.1,Math.max(0,(now-lastFrame)/1000));lastFrame=now;
    if(failed||enginePaused||hidden)return;
    clock+=dt;phase=(phase+dt*config.cavespeed)%256;if(config.orbit)angle+=dt*config.orbitspeed*Math.PI/180;
    if(held){accept(held);held=null;}
    if(simulation&&!engine){const pulse=Math.exp(-(clock*1.8%1)*6);audio={rms:.12+pulse*.25,bass:pulse*.55,mid:.15+.12*Math.sin(clock*1.1),treble:pulse*.2};diagnostics.audio=audio;}
    else if(now-lastAudio>300){audio=MTAudio.zero();diagnostics.audio=audio;diagnostics.input=MTAudio.zero();normalizer.reset();}
    const meshInterval=Math.max(1000/Math.min(config.meshfps,fps),diagnostics.generationMs*2);
    if(ready&&!busy&&(dirty||now-lastJob>=meshInterval)){
      busy=true;dirty=false;lastJob=now;const elapsed=Math.min(.25,Math.max(0,clock-lastJobClock));lastJobClock=clock;
      worker.postMessage({type:'generate',config,audio,phase,dt:elapsed,revision});
    }
    const interval=1000/fps;
    if(now-lastDraw>=interval){lastDraw=now-(now-lastDraw)%interval;renderer.draw(config,angle,clock);diagnostics.frames++;}
  }
  try{
    sync();renderer=new MTRenderer($('world'));worker=new Worker('terrain-worker.js');
    worker.onmessage=event=>{const message=event.data;if(message.type==='ready'){ready=true;return;}if(message.type==='error'){busy=false;fail(message.message);return;}if(message.type==='mesh'){busy=false;if(enginePaused||hidden)held=message;else accept(message);}};
    worker.onerror=event=>fail(event.message||'后台地形线程无法启动');
    document.addEventListener('visibilitychange',()=>{hidden=!engine&&document.hidden;lastFrame=performance.now();diagnostics.paused=enginePaused||hidden;});
    $('settings').onclick=()=>{$('panel').hidden=!$('panel').hidden;};$('close').onclick=()=>{$('panel').hidden=true;};
    for(const input of document.querySelectorAll('[data-property]'))input.addEventListener('input',()=>apply({[input.dataset.property]:{value:input.type==='checkbox'?input.checked:input.value}}));
    $('demo').onclick=()=>{simulation=!simulation;$('demo').textContent=simulation?'关闭模拟音乐':'开启模拟音乐（无声音）';dirty=true;};
    $('world').addEventListener('wheel',event=>{event.preventDefault();apply({zoom:{value:config.zoom*Math.exp(-event.deltaY*.001)}});},{passive:false});
    $('world').addEventListener('webglcontextlost',event=>{event.preventDefault();fail('显卡画面已中断，请重新加载壁纸。');});
    addEventListener('beforeunload',()=>worker.terminate());requestAnimationFrame(frame);
  }catch(error){fail(error);}
})();
