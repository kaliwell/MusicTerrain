(function(root){
  'use strict';
  const vertex=`#version 300 es
  precision highp float;
  layout(location=0) in vec3 position;layout(location=1) in vec2 uv;layout(location=2) in float layer;layout(location=3) in float shade;
  uniform mat4 matrix;out vec2 texUV;flat out float texLayer;out float light;
  void main(){gl_Position=matrix*vec4(position,1.0);texUV=uv;texLayer=layer;light=shade;}`;
  const fragment=`#version 300 es
  precision highp float;precision highp sampler2DArray;
  uniform sampler2DArray atlas;uniform float time;in vec2 texUV;flat in float texLayer;in float light;out vec4 result;
  void main(){vec4 color=texture(atlas,vec3(fract(texUV),texLayer));if(color.a<0.12)discard;
    if(texLayer==5.0)color.rgb*=0.96+0.04*sin(time*1.6+texUV.x*0.25+texUV.y*0.2);
    result=vec4(color.rgb*light,color.a);}`;
  function textures(){
    const size=16,layers=10,data=new Uint8Array(size*size*layers*4);
    const palette=[[91,145,48],[118,83,49],[120,83,50],[126,130,129],[216,204,146],[78,147,191],[111,83,45],[158,125,73],[64,112,40],[239,245,246]];
    for(let layer=0;layer<layers;layer++)for(let y=0;y<size;y++)for(let x=0;x<size;x++){
      const hash=(Math.imul(x+layer*113+17,374761393)^Math.imul(y+47,668265263))>>>0;
      const delta=((hash^(hash>>>13))&31)-15;
      let color=palette[layer],alpha=255;
      if(layer===1&&y>=12-((x*7)%3))color=palette[0];
      if(layer===6)color=palette[layer].map((v,i)=>v+((x%4===0)?-20:6)+(i===0?4:0));
      if(layer===7){const ring=Math.max(Math.abs(x-7.5),Math.abs(y-7.5))|0;color=palette[layer].map(v=>v+(ring%3===0?-22:5));}
      if(layer===8&&hash%11<2)alpha=0;
      if(layer===5)alpha=145;
      const at=((layer*size+y)*size+x)*4;
      for(let i=0;i<3;i++)data[at+i]=Math.max(0,Math.min(255,color[i]+delta));data[at+3]=alpha;
    }
    return {data,size,layers};
  }
  class Renderer{
    constructor(canvas){
      this.canvas=canvas;const gl=canvas.getContext('webgl2',{alpha:true,antialias:true,powerPreference:'low-power'});
      if(!gl)throw new Error('无法创建 WebGL 2 画面，请检查 Wallpaper Engine 的显卡设置。');this.gl=gl;
      const shader=(type,source)=>{const shader=gl.createShader(type);gl.shaderSource(shader,source);gl.compileShader(shader);if(!gl.getShaderParameter(shader,gl.COMPILE_STATUS))throw new Error(gl.getShaderInfoLog(shader));return shader;};
      this.program=gl.createProgram();const vs=shader(gl.VERTEX_SHADER,vertex),fs=shader(gl.FRAGMENT_SHADER,fragment);
      gl.attachShader(this.program,vs);gl.attachShader(this.program,fs);gl.linkProgram(this.program);gl.deleteShader(vs);gl.deleteShader(fs);
      if(!gl.getProgramParameter(this.program,gl.LINK_STATUS))throw new Error(gl.getProgramInfoLog(this.program));
      this.matrix=gl.getUniformLocation(this.program,'matrix');this.time=gl.getUniformLocation(this.program,'time');
      const tex=textures();this.texture=gl.createTexture();gl.bindTexture(gl.TEXTURE_2D_ARRAY,this.texture);
      gl.texImage3D(gl.TEXTURE_2D_ARRAY,0,gl.RGBA8,tex.size,tex.size,tex.layers,0,gl.RGBA,gl.UNSIGNED_BYTE,tex.data);
      gl.texParameteri(gl.TEXTURE_2D_ARRAY,gl.TEXTURE_MIN_FILTER,gl.NEAREST);gl.texParameteri(gl.TEXTURE_2D_ARRAY,gl.TEXTURE_MAG_FILTER,gl.NEAREST);
      gl.texParameteri(gl.TEXTURE_2D_ARRAY,gl.TEXTURE_WRAP_S,gl.REPEAT);gl.texParameteri(gl.TEXTURE_2D_ARRAY,gl.TEXTURE_WRAP_T,gl.REPEAT);
      this.parts=[this.part(),this.part()];gl.enable(gl.DEPTH_TEST);gl.enable(gl.CULL_FACE);gl.clearColor(0,0,0,0);
    }
    part(){const gl=this.gl,vao=gl.createVertexArray(),buffer=gl.createBuffer();gl.bindVertexArray(vao);gl.bindBuffer(gl.ARRAY_BUFFER,buffer);
      for(const [location,count,offset] of [[0,3,0],[1,2,12],[2,1,20],[3,1,24]]){gl.enableVertexAttribArray(location);gl.vertexAttribPointer(location,count,gl.FLOAT,false,28,offset);}
      return {vao,buffer,count:0};}
    upload(mesh){const gl=this.gl;for(let i=0;i<2;i++){const data=i===0?mesh.solid:mesh.water;gl.bindBuffer(gl.ARRAY_BUFFER,this.parts[i].buffer);gl.bufferData(gl.ARRAY_BUFFER,data,gl.DYNAMIC_DRAW);this.parts[i].count=data.length/7;}}
    draw(config,angle,time){
      const gl=this.gl,scale=Math.min(devicePixelRatio||1,2)*config.renderscale;
      const width=Math.max(1,Math.round(innerWidth*scale)),height=Math.max(1,Math.round(innerHeight*scale));
      if(this.canvas.width!==width||this.canvas.height!==height){this.canvas.width=width;this.canvas.height=height;gl.viewport(0,0,width,height);}
      const radius=config.size*1.7/config.zoom,pitch=.58;
      const eye=[Math.sin(angle)*radius*Math.cos(pitch),16+radius*Math.sin(pitch),Math.cos(angle)*radius*Math.cos(pitch)];
      const projection=config.orthographic?MTMath.orthographic(config.size*1.45/config.zoom,width/height,.1,2000):MTMath.perspective(.73,width/height,.1,2000);
      const matrix=MTMath.multiply(projection,MTMath.view(eye,[0,16,0]));
      gl.clear(gl.COLOR_BUFFER_BIT|gl.DEPTH_BUFFER_BIT);gl.useProgram(this.program);gl.uniformMatrix4fv(this.matrix,false,matrix);gl.uniform1f(this.time,time);
      gl.activeTexture(gl.TEXTURE0);gl.bindTexture(gl.TEXTURE_2D_ARRAY,this.texture);
      gl.disable(gl.BLEND);gl.depthMask(true);gl.bindVertexArray(this.parts[0].vao);gl.drawArrays(gl.TRIANGLES,0,this.parts[0].count);
      gl.enable(gl.BLEND);gl.blendFuncSeparate(gl.SRC_ALPHA,gl.ONE_MINUS_SRC_ALPHA,gl.ONE,gl.ONE_MINUS_SRC_ALPHA);gl.depthMask(false);gl.bindVertexArray(this.parts[1].vao);gl.drawArrays(gl.TRIANGLES,0,this.parts[1].count);gl.depthMask(true);
    }
  }
  root.MTRenderer=Renderer;
})(globalThis);
