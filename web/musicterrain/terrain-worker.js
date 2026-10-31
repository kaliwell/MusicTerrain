'use strict';
importScripts('kernel-base64.js');
let wasm;
const bytes=Uint8Array.from(atob(self.MusicTerrainKernel),c=>c.charCodeAt(0));
self.MusicTerrainKernel=null;
const ready=WebAssembly.instantiate(bytes,{}).then(result=>{wasm=result.instance.exports;postMessage({type:'ready'});});
ready.catch(error=>postMessage({type:'error',message:String(error)}));
onmessage=async event=>{
  const request=event.data;if(request.type!=='generate')return;
  try{
    await ready;const start=performance.now(),c=request.config,a=request.audio;
    const flags=(c.caves?1:0)|(c.trees?2:0)|(c.water?4:0);
    if(wasm.terrain_generate(c.size,c.shape,c.noise,c.seed,request.phase,request.dt,a.rms,a.bass,a.mid,a.treble,c.intensity,flags)!==1)throw new Error('地形输入无效或已超过内存预算');
    const solid=new Float32Array(wasm.memory.buffer,wasm.terrain_solid_ptr(),wasm.terrain_solid_len()).slice();
    const water=new Float32Array(wasm.memory.buffer,wasm.terrain_water_ptr(),wasm.terrain_water_len()).slice();
    postMessage({type:'mesh',revision:request.revision,solid,water,carved:wasm.terrain_carved(),milliseconds:performance.now()-start},[solid.buffer,water.buffer]);
  }catch(error){postMessage({type:'error',message:String(error)});}
};
