// Capture the shipped Rust worker and WebGL renderer at deterministic timestamps.
// Test rhythm only: no recording of the user's audio or desktop.
const fs=require('node:fs'),path=require('node:path'),http=require('node:http');
const {chromium}=require('../run/validation/web-tools/node_modules/playwright');
const root=path.resolve(__dirname,'../dist/MusicTerrain-Web');
const frames=path.resolve(__dirname,'../run/validation/promo-frames');
fs.mkdirSync(frames,{recursive:true});
const html=`<!doctype html><meta charset="utf-8"><style>
html,body{margin:0;overflow:hidden;background:radial-gradient(ellipse at 50% 25%,#c1e3fa,#abcfee 85%)}canvas{display:block;width:100vw;height:100vh}</style>
<canvas id="world"></canvas><script src="math.js"></script><script src="renderer.js"></script><script>
const renderer=new MTRenderer(document.getElementById('world')),worker=new Worker('terrain-worker.js');
const config={size:64,shape:1,noise:3,seed:42,intensity:.55,caves:true,trees:true,water:true,biome:'forest',villages:'showcase',mines:'showcase',view:'surface',full_detail:true,lab:false,zoom:1.17,pitch:.58,panx:0,pany:17,panz:0,renderscale:1,orthographic:false};
let resolveFrame;
window.ready=new Promise(resolve=>worker.onmessage=e=>{
 const m=e.data;if(m.type==='ready'){renderer.setAtlas(m.atlas);const preset=m.metadata.presets[0];config.parameters={...preset.parameters,base_height:22,amplitude:26};config.mappings=preset.mappings;resolve();}
 else if(m.type==='mesh'){renderer.upload(m);window.scene=m.stats;resolveFrame(m);}
 else throw new Error(m.message);
});
window.captureFrame=async t=>{
 const pulse=.5+.5*Math.sin(t*Math.PI*3),a={rms:.10+pulse*.13,bass:.12+pulse*.18,mid:.10+.07*Math.sin(t*Math.PI*2+.7),treble:.05+pulse*.08};
 await new Promise(resolve=>{resolveFrame=resolve;worker.postMessage({type:'generate',config,audio:a,phase:Math.sin(t*Math.PI/6)*.6,clock:t,dt:1/15,revision:1});});
 renderer.draw(config,.9+t*Math.PI/6,t);await new Promise(requestAnimationFrame);
};</script>`;
const server=http.createServer((req,res)=>{
 const name=new URL(req.url,'http://localhost').pathname;
 if(name==='/promo.html'){res.setHeader('Content-Type','text/html; charset=utf-8');return res.end(html);}
 const file=path.resolve(root,'.'+name);if(!file.startsWith(root+path.sep)){res.writeHead(403).end();return;}
 fs.readFile(file,(error,data)=>{if(error){res.writeHead(404).end();return;}res.setHeader('Content-Type',path.extname(file)==='.js'?'application/javascript':'application/octet-stream');res.end(data);});
});
(async()=>{
 await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
 const browser=await chromium.launch({channel:'msedge',headless:true,args:['--mute-audio']});
 try{
  const page=await browser.newPage({viewport:{width:960,height:640},deviceScaleFactor:1});
  const errors=[];page.on('pageerror',error=>errors.push(String(error)));
  await page.goto('http://127.0.0.1:'+server.address().port+'/promo.html');await page.evaluate(()=>ready);
  // Pre-roll the shared attack/release mapper to its periodic response.
  for(let i=0;i<90;i++)await page.evaluate(t=>captureFrame(t),i/15);
  for(let i=0;i<180;i++){
   await page.evaluate(t=>captureFrame(t),12+i/15);
   await page.screenshot({path:path.join(frames,String(i).padStart(4,'0')+'.png')});
   if(i%30===0)console.log('Captured '+i+'/180');
  }
  const result=await page.evaluate(()=>({scene,glError:document.getElementById('world').getContext('webgl2').getError()}));
  if(errors.length||result.glError)throw new Error(JSON.stringify({errors,...result}));
  fs.writeFileSync(path.join(frames,'capture.json'),JSON.stringify({fps:15,duration:12,width:960,height:640,input:'synthetic rhythm; no recorded audio',...result},null,2));
  console.log('Capture complete: '+frames);
 }finally{await browser.close();server.close();}
})().catch(error=>{console.error(error);server.close();process.exitCode=1;});
