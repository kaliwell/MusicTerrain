// Integration verification in a fresh Edge context. Does not access user tabs.
const fs=require('node:fs');
const path=require('node:path');
const http=require('node:http');
const assert=require('node:assert/strict');
let chromium;
try{({chromium}=require('playwright'));}catch{({chromium}=require('../run/validation/web-tools/node_modules/playwright'));}
const root=path.resolve(__dirname,'../dist/MusicTerrain-Web');
const evidence=path.resolve(__dirname,'../run/validation/web-browser');fs.mkdirSync(evidence,{recursive:true});
const types={'.html':'text/html; charset=utf-8','.js':'text/javascript; charset=utf-8','.css':'text/css; charset=utf-8','.jpg':'image/jpeg'};
const server=http.createServer((request,response)=>{
 const name=decodeURIComponent(new URL(request.url,'http://localhost').pathname);
 const file=path.resolve(root,'.'+(name==='/'?'/index.html':name));
 if(!file.startsWith(root+path.sep)){response.writeHead(403).end();return;}
 fs.readFile(file,(error,data)=>{if(error){response.writeHead(404).end();return;}response.setHeader('Content-Type',types[path.extname(file)]||'application/octet-stream');response.end(data);});
});
(async()=>{
 await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
 const url=`http://127.0.0.1:${server.address().port}/`;
 const browser=await chromium.launch({channel:'msedge',headless:true,args:['--mute-audio']});
 const errors=[];const results={};
 try{
  const page=await browser.newPage({viewport:{width:1280,height:800}});
  page.on('pageerror',error=>errors.push(String(error)));page.on('console',message=>{if(message.type()==='error')errors.push(message.text());});
  await page.goto(url);await page.waitForFunction(()=>window.MusicTerrain?.diagnostics.meshes>=2,{timeout:30000});
  assert.equal(await page.locator('#error').isVisible(),false);
  await page.screenshot({path:path.join(evidence,'browser-preview.png')});
  results.preview=await page.evaluate(()=>({...MusicTerrain.diagnostics}));await page.close();
  const engine=await browser.newPage({viewport:{width:1600,height:900}});
  await engine.addInitScript(()=>{window.wallpaperRegisterAudioListener=callback=>{window.__audioCallback=callback;window.__registrations=(window.__registrations||0)+1;};});
  engine.on('pageerror',error=>errors.push(String(error)));engine.on('console',message=>{if(message.type()==='error')errors.push(message.text());});
  await engine.goto(url);await engine.waitForFunction(()=>window.MusicTerrain?.diagnostics.meshes>=2,{timeout:30000});
  assert.equal(await engine.evaluate(()=>window.__registrations),1);
  assert.equal(await engine.locator('#notice').isVisible(),false);
  await engine.evaluate(()=>wallpaperPropertyListener.applyUserProperties({orbit:{value:false},cavespeed:{value:0}}));
  await engine.waitForTimeout(600);
  const quiet=await engine.evaluate(()=>MusicTerrain.diagnostics.vertices);
  await engine.evaluate(()=>{window.__audioTimer=setInterval(()=>{const bins=new Array(128).fill(0);for(const i of [2,4,66,68])bins[i]=.002;__audioCallback(bins);},33);});
  await engine.waitForFunction(quiet=>MusicTerrain.diagnostics.input.rms<.001&&MusicTerrain.diagnostics.audio.bass>.3&&MusicTerrain.diagnostics.vertices!==quiet,quiet,{timeout:15000});
  results.live=await engine.evaluate(()=>({...MusicTerrain.diagnostics}));
  await engine.screenshot({path:path.join(evidence,'audio-reactive.png')});
  for(let shape=0;shape<4;shape++){
   const previous=await engine.evaluate(()=>MusicTerrain.diagnostics.meshes);
   await engine.evaluate(shape=>wallpaperPropertyListener.applyUserProperties({shape:{value:String(shape)},orthographic:{value:shape%2===1}}),shape);
   await engine.waitForFunction(previous=>MusicTerrain.diagnostics.meshes>=previous+2,previous,{timeout:20000});
   assert.equal(await engine.locator('#error').isVisible(),false);
   await engine.screenshot({path:path.join(evidence,`shape-${shape}.png`)});
  }
  await engine.evaluate(()=>wallpaperPropertyListener.setPaused(true));
  const frozen=await engine.evaluate(()=>({frames:MusicTerrain.diagnostics.frames,meshes:MusicTerrain.diagnostics.meshes}));
  await engine.waitForTimeout(400);
  assert.deepEqual(await engine.evaluate(()=>({frames:MusicTerrain.diagnostics.frames,meshes:MusicTerrain.diagnostics.meshes})),frozen);
  await engine.evaluate(()=>wallpaperPropertyListener.setPaused(false));
  await engine.waitForFunction(frames=>MusicTerrain.diagnostics.frames>frames,frozen.frames);
  await engine.evaluate(()=>{clearInterval(__audioTimer);__audioCallback(new Array(128).fill(0));wallpaperPropertyListener.applyGeneralProperties({fps:10});});
  await engine.waitForTimeout(350);
  assert.equal(await engine.evaluate(()=>MusicTerrain.diagnostics.audio.rms),0);
  const startFrames=await engine.evaluate(()=>MusicTerrain.diagnostics.frames);
  await engine.waitForTimeout(1000);
  const frames=await engine.evaluate(()=>MusicTerrain.diagnostics.frames)-startFrames;
  assert.ok(frames>=6&&frames<=13,`FPS cap: ${frames}`);results.fpsCapFrames=frames;
  await engine.evaluate(()=>wallpaperPropertyListener.applyUserProperties({size:{value:'128'},shape:{value:'2'}}));
  const baseline=await engine.evaluate(()=>MusicTerrain.diagnostics.meshes);
  await engine.waitForFunction(baseline=>MusicTerrain.diagnostics.meshes>=baseline+2,baseline,{timeout:30000});
  results.large=await engine.evaluate(()=>({...MusicTerrain.diagnostics}));
  await engine.setViewportSize({width:1680,height:720});await engine.waitForTimeout(400);
  await engine.screenshot({path:path.join(evidence,'ultrawide-128.png')});
  // Exercise the actual native scene, editor, slices and PCM path in WASM.
  await engine.evaluate(()=>MusicTerrain.edit({size:64,biome:'jungle',villages:'showcase',mines:'showcase',view:'cutaway',showcontrols:true}));
  let n=await engine.evaluate(()=>MusicTerrain.diagnostics.meshes);
  await engine.waitForFunction(n=>MusicTerrain.diagnostics.meshes>n,n);
  let scene=await engine.evaluate(()=>MusicTerrain.diagnostics.scene);
  assert.ok(scene.houses>0&&scene.mines>0&&scene.rails>0);
  assert.equal(scene.materials.length,65);assert.ok(scene.materials.slice(10).some(v=>v>0));
  await engine.click('#settings');await engine.getByRole('button',{name:'噪波实验室',exact:true}).click();
  await engine.waitForFunction(()=>MusicTerrain.config.lab&&document.querySelector('canvas.noise').getContext('2d').getImageData(20,20,1,1).data[3]===255);
  await engine.screenshot({path:path.join(evidence,'native-scene-lab.png')});
  await engine.getByRole('button',{name:'预设 / 文件',exact:true}).click();
  for(let i=0;i<7;i++){
   n=await engine.evaluate(()=>MusicTerrain.diagnostics.meshes);
   await engine.evaluate(i=>MTEditor.loadPreset(MusicTerrain.metadata.presets[i]),i);
   await engine.waitForFunction(n=>MusicTerrain.diagnostics.meshes>n,n,{timeout:30000});
   assert.equal(await engine.locator('#error').isVisible(),false);
  }
  assert.equal(await engine.locator('#mappings details').count(),await engine.evaluate(()=>MusicTerrain.config.mappings.rules.length));
  await engine.evaluate(()=>{window.__validParameters={...MusicTerrain.config.parameters};MusicTerrain.edit({parameters:{...__validParameters,frequency:-1}});});
  await engine.waitForFunction(()=>Boolean(MusicTerrain.diagnostics.validationError));
  const stillDrawing=await engine.evaluate(()=>MusicTerrain.diagnostics.frames);await engine.waitForTimeout(250);
  assert.ok(await engine.evaluate(n=>MusicTerrain.diagnostics.frames>n,stillDrawing));assert.equal(await engine.locator('#error').isVisible(),false);
  n=await engine.evaluate(()=>MusicTerrain.diagnostics.meshes);await engine.evaluate(()=>MusicTerrain.edit({parameters:__validParameters}));
  await engine.waitForFunction(n=>MusicTerrain.diagnostics.meshes>n&&!MusicTerrain.diagnostics.validationError,n);
  const wav=Buffer.alloc(44+48000*2);wav.write('RIFF');wav.writeUInt32LE(wav.length-8,4);wav.write('WAVEfmt ',8);wav.writeUInt32LE(16,16);wav.writeUInt16LE(1,20);wav.writeUInt16LE(1,22);wav.writeUInt32LE(48000,24);wav.writeUInt32LE(96000,28);wav.writeUInt16LE(2,32);wav.writeUInt16LE(16,34);wav.write('data',36);wav.writeUInt32LE(wav.length-44,40);
  for(let i=0;i<48000;i++)wav.writeInt16LE(Math.round(Math.sin(i*2*Math.PI*500/48000)*12000),44+i*2);
  await engine.locator('input[accept=".mp3,.wav,.flac,.ogg"]').setInputFiles({name:'synthetic-500Hz.wav',mimeType:'audio/wav',buffer:wav});
  await engine.evaluate(async()=>{const p=document.getElementById('music-player');p.loop=true;await p.play();});
  await engine.waitForFunction(()=>MusicTerrain.diagnostics.scene.features.rms>.1,{timeout:15000});
  scene=await engine.evaluate(()=>MusicTerrain.diagnostics.scene);assert.ok(scene.features.bands[2]>.1||scene.features.bands[3]>.1);results.nativeFeatures=scene.features;
  await engine.getByRole('button',{name:'切回系统音乐',exact:true}).click();
  await engine.evaluate(()=>{MTEditor.loadPreset(MusicTerrain.metadata.presets[0]);MusicTerrain.edit({biome:'auto',view:'surface'});});
  await engine.click('#close');
  await engine.evaluate(()=>wallpaperPropertyListener.applyUserProperties({size:{value:'64'},shape:{value:'1'},orthographic:{value:false},zoom:{value:1.2}}));
  const posterStart=await engine.evaluate(()=>MusicTerrain.diagnostics.meshes);await engine.waitForFunction(n=>MusicTerrain.diagnostics.meshes>=n+2,posterStart);
  await engine.setViewportSize({width:1280,height:720});await engine.waitForTimeout(400);
  const poster=process.argv.includes('--update-preview')?path.resolve(__dirname,'../web/musicterrain/preview.jpg'):path.join(evidence,'preview.jpg');
  await engine.screenshot({path:poster,type:'jpeg',quality:90});
  assert.deepEqual(errors,[]);results.errors=errors;
  fs.writeFileSync(path.join(evidence,'results.json'),JSON.stringify(results,null,2));
  console.log(JSON.stringify({passed:true,previewMs:results.preview.generationMs,liveMs:results.live.generationMs,largeMs:results.large.generationMs,fpsCapFrames:frames,evidence},null,2));
 }finally{await browser.close();server.close();}
})().catch(error=>{console.error(error);server.close();process.exitCode=1;});
