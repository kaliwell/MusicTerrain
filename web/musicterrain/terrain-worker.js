 'use strict';
importScripts('kernel-base64.js');
let wasm;
const bytes=Uint8Array.from(atob(self.MusicTerrainKernel),c=>c.charCodeAt(0));self.MusicTerrainKernel=null;
const json=()=>JSON.parse(new TextDecoder().decode(new Uint8Array(wasm.memory.buffer,wasm.terrain_json_ptr(),wasm.terrain_json_len())));
const ready=WebAssembly.instantiate(bytes,{}).then(result=>{
  wasm=result.instance.exports;wasm.terrain_metadata();const metadata=json();
  const atlas=new Uint8Array(wasm.memory.buffer,wasm.terrain_atlas_ptr(),wasm.terrain_atlas_len()).slice();
  postMessage({type:'ready',metadata,atlas},[atlas.buffer]);
});
ready.catch(error=>postMessage({type:'error',message:String(error)}));
onmessage=async event=>{
  const request=event.data;if(request.type!=='generate')return;
  try{
    await ready;const start=performance.now(),a=request.audio;
    const features=request.features||{time:request.clock||0,rms:a.rms,bands:[a.bass,a.bass,a.mid,a.mid,a.treble,a.treble,a.treble],centroid:0,flux:0,onset:false,beat_strength:0,beat_phase:null,silence:a.rms===0};
    const input=new TextEncoder().encode(JSON.stringify({...request.config,features,phase:request.phase,dt:request.dt,clock:request.clock||0,pcm:request.pcm||[]}));
    const pointer=wasm.terrain_input(input.length);if(!pointer)throw new Error('?????? 1 MiB');
    new Uint8Array(wasm.memory.buffer,pointer,input.length).set(input);
    if(wasm.terrain_run()!==1){postMessage({type:'invalid',revision:request.revision,message:json().error});return;}
    const solid=new Uint8Array(wasm.memory.buffer,wasm.terrain_solid_ptr(),wasm.terrain_solid_len()).slice();
    const water=new Uint8Array(wasm.memory.buffer,wasm.terrain_water_ptr(),wasm.terrain_water_len()).slice();
    const lab=new Uint8ClampedArray(wasm.memory.buffer,wasm.terrain_lab_ptr(),wasm.terrain_lab_len()).slice();
    postMessage({type:'mesh',revision:request.revision,solid,water,lab,stats:json(),carved:wasm.terrain_carved(),milliseconds:performance.now()-start},[solid.buffer,water.buffer,lab.buffer]);
  }catch(error){postMessage({type:'error',message:String(error)});}
};
