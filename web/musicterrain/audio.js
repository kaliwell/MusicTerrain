(function(root){
  'use strict';
  const zero=()=>({rms:0,bass:0,mid:0,treble:0});
  function spectrum(values){
    if(!values||values.length!==128)return zero();
    const mono=new Float32Array(64);
    let energy=0;
    for(let i=0;i<64;i++){
      const left=Number(values[i]),right=Number(values[i+64]);
      const a=Number.isFinite(left)?Math.max(0,Math.min(1,left)):0;
      const b=Number.isFinite(right)?Math.max(0,Math.min(1,right)):0;
      mono[i]=(a+b)*.5;energy+=mono[i]*mono[i];
    }
    const band=(a,b)=>{let sum=0;for(let i=a;i<b;i++)sum+=mono[i]*mono[i];return Math.sqrt(sum/(b-a));};
    // WE supplies spectrum magnitudes, not PCM. This is a level estimate;
    // these grouped ranges are not claims of calibrated frequency/RMS readings.
    return {rms:Math.sqrt(energy/64),bass:band(0,12),mid:band(12,40),treble:band(40,64)};
  }
  function createNormalizer(){
    let envelope=0,lastTime=null,silence=0;
    return {
      reset(){envelope=0;lastTime=null;silence=0;},
      update(levels,time,sensitivity=1,automatic=true){
        const dt=lastTime===null?1/30:Math.max(0,Math.min(.25,time-lastTime));lastTime=time;
        const peak=Math.max(levels.rms,levels.bass,levels.mid,levels.treble);
        if(peak<=1e-5){silence+=dt;if(silence>.3)envelope=0;return zero();}
        silence=0;
        if(envelope===0)envelope=peak;
        const tau=peak>envelope?.08:1.5;
        envelope+=(peak-envelope)*(1-Math.exp(-dt/tau));
        // Normalize to the shared mapper's useful range. A slow release retains
        // beat dynamics instead of normalizing every individual frame to a peak.
        const gain=sensitivity*(automatic?.45/Math.max(.0005,envelope):1);
        return Object.fromEntries(Object.entries(levels).map(([k,v])=>[k,Math.max(0,Math.min(1,v*gain))]));
      }
    };
  }
  function clean(properties,current){
    const next={...current};
    const numeric={size:[32,256],shape:[0,3],noise:[0,7],seed:[0,65535],intensity:[0,2],sensitivity:[.2,4],cavespeed:[0,1],zoom:[.5,2],meshfps:[4,30],orbitspeed:[0,25],renderscale:[.5,1.5]};
    for(const [key,bounds] of Object.entries(numeric)){
      if(!properties[key])continue;
      const value=Number(properties[key].value);if(Number.isFinite(value))next[key]=Math.max(bounds[0],Math.min(bounds[1],value));
    }
    next.size=Math.round(next.size/16)*16;
    for(const key of ['shape','noise','seed'])next[key]=Math.round(next[key]);
    for(const key of ['orbit','orthographic','caves','trees','water','showcontrols','autogain','full_detail']){
      if(properties[key]&&typeof properties[key].value==='boolean')next[key]=properties[key].value;
    }
    for(const [key,allowed] of Object.entries({biome:['auto','meadow','forest','alpine','shore','birch-forest','taiga','desert','badlands','savanna','swamp','jungle'],villages:['auto','showcase','off'],mines:['auto','showcase','off'],view:['surface','cutaway','underground']})){
      if(allowed.includes(properties[key]?.value))next[key]=properties[key].value;
    }
    if(properties.background&&typeof properties.background.value==='string'){
      const parts=properties.background.value.trim().split(/\s+/).map(Number);
      if(parts.length===3&&parts.every(Number.isFinite))next.background=parts.map(x=>Math.max(0,Math.min(1,x)));
    }
    return next;
  }
  let previous=new Array(7).fill(0),average=0,lastOnset=-Infinity,onsets=[];
  function features(values,levels,time){
    const cuts=[0,4,10,18,30,42,54,64],raw=spectrum(values),gain=raw.rms>1e-8?levels.rms/raw.rms:0;
    const mono=Array.from({length:64},(_,i)=>{
      const sample=j=>Number.isFinite(Number(values?.[j]))?Math.max(0,Math.min(1,Number(values[j]))):0;
      return Math.min(1,(sample(i)+sample(i+64))*.5*gain);
    });
    const bands=cuts.slice(0,7).map((start,i)=>Math.sqrt(mono.slice(start,cuts[i+1]).reduce((s,v)=>s+v*v,0)/(cuts[i+1]-start)));
    const flux=bands.reduce((s,v,i)=>s+Math.max(0,v-previous[i]),0)/7;previous=bands;
    average=average*.95+flux*.05;const onset=levels.rms>1e-5&&flux>Math.max(.008,average*1.5)&&time-lastOnset>.18;
    if(onset){lastOnset=time;onsets.push(time);onsets=onsets.slice(-5);}
    const intervals=onsets.slice(1).map((v,i)=>v-onsets[i]),period=intervals.reduce((a,b)=>a+b,0)/Math.max(1,intervals.length);
    const stable=intervals.length>=3&&period>.25&&period<2&&intervals.every(v=>Math.abs(v-period)<period*.2)&&time-lastOnset<period*2;
    const sum=mono.reduce((a,b)=>a+b,0);
    return {time,rms:levels.rms,bands,centroid:sum?mono.reduce((s,v,i)=>s+v*(i+.5)*20000/64,0)/sum:0,flux,onset,beat_strength:onset?Math.min(1,flux/Math.max(.01,average)):0,beat_phase:stable?((time-lastOnset)/period)%1:null,silence:levels.rms<=1e-5};
  }
  const api={spectrum,clean,zero,createNormalizer,features};root.MTAudio=api;
  if(typeof module!=='undefined')module.exports=api;
})(globalThis);
