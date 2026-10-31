(function(root){
  'use strict';
  const normalize=v=>{const length=Math.hypot(...v)||1;return v.map(x=>x/length);};
  const cross=(a,b)=>[a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0]];
  const dot=(a,b)=>a.reduce((s,x,i)=>s+x*b[i],0);
  function view(eye,target){
    const z=normalize(eye.map((x,i)=>x-target[i]));const x=normalize(cross([0,1,0],z));const y=cross(z,x);
    return new Float32Array([x[0],y[0],z[0],0,x[1],y[1],z[1],0,x[2],y[2],z[2],0,-dot(x,eye),-dot(y,eye),-dot(z,eye),1]);
  }
  function perspective(fov,aspect,near,far){const f=1/Math.tan(fov/2),nf=1/(near-far);return new Float32Array([f/aspect,0,0,0,0,f,0,0,0,0,(far+near)*nf,-1,0,0,2*far*near*nf,0]);}
  function orthographic(span,aspect,near,far){return new Float32Array([2/(span*aspect),0,0,0,0,2/span,0,0,0,0,-2/(far-near),0,0,0,-(far+near)/(far-near),1]);}
  function multiply(a,b){const out=new Float32Array(16);for(let c=0;c<4;c++)for(let r=0;r<4;r++)for(let k=0;k<4;k++)out[c*4+r]+=a[k*4+r]*b[c*4+k];return out;}
  const api={view,perspective,orthographic,multiply};root.MTMath=api;if(typeof module!=='undefined')module.exports=api;
})(globalThis);
