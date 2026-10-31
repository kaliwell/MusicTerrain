// Browser-independent tests for the WE bridge; no audio capture or network.
const test=require('node:test');
const assert=require('node:assert/strict');
const {spectrum,clean,createNormalizer}=require('../web/musicterrain/audio.js');
const math=require('../web/musicterrain/math.js');
test('WE stereo spectra are grouped, bounded and silent-safe',()=>{
 assert.deepEqual(spectrum(new Array(128).fill(0)),{rms:0,bass:0,mid:0,treble:0});
 const stereo=new Array(128).fill(0);for(let i=0;i<12;i++){stereo[i]=.4;stereo[i+64]=.8;}
 assert.ok(Math.abs(spectrum(stereo).bass-.6)<1e-6);assert.equal(spectrum(stereo).treble,0);
 assert.equal(spectrum(new Array(128).fill(Infinity)).rms,0);
 assert.equal(spectrum(new Array(128).fill(10)).rms,1);
 assert.equal(spectrum([1,2]).rms,0);
});
test('partial WE properties retain other settings and reject malformed values',()=>{
 const defaults={size:64,shape:1,noise:2,seed:42,zoom:1,orbit:true};
 const next=clean({size:{value:'999'},shape:{value:'2'},zoom:{value:'bad'},orbit:{value:false}},defaults);
 assert.equal(next.size,256);assert.equal(next.shape,2);assert.equal(next.zoom,1);assert.equal(next.seed,42);assert.equal(next.orbit,false);
});
test('quiet sparse music reaches the mapper while silence stays zero',()=>{
 const bins=new Array(128).fill(0);for(const i of [2,4,66,68])bins[i]=.002;
 const raw=spectrum(bins);assert.ok(raw.bass<.001);
 const normalizer=createNormalizer();const active=normalizer.update(raw,0);
 assert.ok(active.bass>.4&&active.bass<.5);
 assert.equal(normalizer.update(spectrum(new Array(128).fill(0)),.033).rms,0);
 const softer=createNormalizer().update(spectrum(bins.map(x=>x*.5)),0);
 assert.ok(softer.bass>.3);
 const stronger=createNormalizer().update(raw,0,2);assert.ok(stronger.bass>active.bass);
 const transient=normalizer.update(spectrum(bins.map(x=>x*.2)),.066);
 assert.ok(transient.bass<active.bass*.5,'release envelope must preserve beat dynamics');
});
test('camera projects the target into the center in both projection modes',()=>{
 const view=math.view([60,60,80],[0,16,0]);
 for(const projection of [math.perspective(.73,16/9,.1,2000),math.orthographic(90,16/9,.1,2000)]){
  const m=math.multiply(projection,view);const p=[0,16,0,1];
  const result=[0,1,2,3].map(r=>p.reduce((sum,v,c)=>sum+m[c*4+r]*v,0));
  assert.ok(Math.abs(result[0]/result[3])<1e-5);assert.ok(Math.abs(result[1]/result[3])<1e-5);
  assert.ok(result[2]/result[3]>-1&&result[2]/result[3]<1);
 }
});
