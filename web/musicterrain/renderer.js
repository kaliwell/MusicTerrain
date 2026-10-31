(function(root){
  'use strict';
  const vertex=`#version 300 es
  precision highp float;
  layout(location=0) in vec3 position;layout(location=1) in vec2 uv;layout(location=2) in float layer;layout(location=3) in vec3 tint;
  uniform mat4 matrix;out vec2 texUV;flat out float texLayer;out vec3 light;
  void main(){gl_Position=matrix*vec4(position,1.0);texUV=uv;texLayer=layer;light=tint;}`;
  const fragment=`#version 300 es
  precision highp float;precision highp sampler2DArray;
  uniform sampler2DArray atlas;uniform float time;in vec2 texUV;flat in float texLayer;in vec3 light;out vec4 result;
  void main(){vec4 color=texture(atlas,vec3(fract(texUV),texLayer));if(color.a<0.12)discard;
    if(texLayer==5.0)color.rgb*=0.96+0.04*sin(time*1.6+texUV.x*0.25+texUV.y*0.2);
    result=vec4(color.rgb*light,color.a);}`;
  class Renderer{
    constructor(canvas){
      this.canvas=canvas;const gl=canvas.getContext('webgl2',{alpha:true,antialias:true,powerPreference:'low-power'});
      if(!gl)throw new Error('无法创建 WebGL 2 画面，请检查 Wallpaper Engine 的显卡设置。');this.gl=gl;
      const shader=(type,source)=>{const shader=gl.createShader(type);gl.shaderSource(shader,source);gl.compileShader(shader);if(!gl.getShaderParameter(shader,gl.COMPILE_STATUS))throw new Error(gl.getShaderInfoLog(shader));return shader;};
      this.program=gl.createProgram();const vs=shader(gl.VERTEX_SHADER,vertex),fs=shader(gl.FRAGMENT_SHADER,fragment);
      gl.attachShader(this.program,vs);gl.attachShader(this.program,fs);gl.linkProgram(this.program);gl.deleteShader(vs);gl.deleteShader(fs);
      if(!gl.getProgramParameter(this.program,gl.LINK_STATUS))throw new Error(gl.getProgramInfoLog(this.program));
      this.matrix=gl.getUniformLocation(this.program,'matrix');this.time=gl.getUniformLocation(this.program,'time');
      this.texture=gl.createTexture();
      this.parts=[this.part(),this.part()];gl.enable(gl.DEPTH_TEST);gl.enable(gl.CULL_FACE);gl.clearColor(0,0,0,0);
    }
    setAtlas(data,size=32,layers=65){const gl=this.gl;gl.bindTexture(gl.TEXTURE_2D_ARRAY,this.texture);
      gl.texImage3D(gl.TEXTURE_2D_ARRAY,0,gl.RGBA8,size,size,layers,0,gl.RGBA,gl.UNSIGNED_BYTE,data);
      gl.texParameteri(gl.TEXTURE_2D_ARRAY,gl.TEXTURE_MIN_FILTER,gl.NEAREST);gl.texParameteri(gl.TEXTURE_2D_ARRAY,gl.TEXTURE_MAG_FILTER,gl.NEAREST);
      gl.texParameteri(gl.TEXTURE_2D_ARRAY,gl.TEXTURE_WRAP_S,gl.REPEAT);gl.texParameteri(gl.TEXTURE_2D_ARRAY,gl.TEXTURE_WRAP_T,gl.REPEAT);
    }
    part(){const gl=this.gl,vao=gl.createVertexArray(),buffer=gl.createBuffer();gl.bindVertexArray(vao);gl.bindBuffer(gl.ARRAY_BUFFER,buffer);
      for(const [location,count,offset] of [[0,3,0],[1,2,12],[2,1,20],[3,3,24]]){gl.enableVertexAttribArray(location);gl.vertexAttribPointer(location,count,gl.FLOAT,false,36,offset);}
      return {vao,buffer,count:0};}
    upload(mesh){const gl=this.gl;for(let i=0;i<2;i++){const data=i===0?mesh.solid:mesh.water;gl.bindBuffer(gl.ARRAY_BUFFER,this.parts[i].buffer);gl.bufferData(gl.ARRAY_BUFFER,data,gl.DYNAMIC_DRAW);this.parts[i].count=data.length/9;}}
    draw(config,angle,time){
      const gl=this.gl,scale=Math.min(devicePixelRatio||1,2)*config.renderscale;
      const width=Math.max(1,Math.round(innerWidth*scale)),height=Math.max(1,Math.round(innerHeight*scale));
      if(this.canvas.width!==width||this.canvas.height!==height){this.canvas.width=width;this.canvas.height=height;gl.viewport(0,0,width,height);}
      const radius=Math.max(config.size*1.9,145)/config.zoom,pitch=config.pitch||.58;
      const eye=[Math.sin(angle)*radius*Math.cos(pitch),radius*Math.sin(pitch),Math.cos(angle)*radius*Math.cos(pitch)];
      const projection=config.orthographic?MTMath.orthographic(Math.max(config.size*1.5,120)/config.zoom,width/height,.1,2000):MTMath.perspective(.73,width/height,.1,2000);
      const matrix=MTMath.multiply(projection,MTMath.view(eye,[0,0,0]));
      gl.clear(gl.COLOR_BUFFER_BIT|gl.DEPTH_BUFFER_BIT);gl.useProgram(this.program);gl.uniformMatrix4fv(this.matrix,false,matrix);gl.uniform1f(this.time,time);
      gl.activeTexture(gl.TEXTURE0);gl.bindTexture(gl.TEXTURE_2D_ARRAY,this.texture);
      gl.disable(gl.BLEND);gl.depthMask(true);gl.bindVertexArray(this.parts[0].vao);gl.drawArrays(gl.TRIANGLES,0,this.parts[0].count);
      gl.enable(gl.BLEND);gl.blendFuncSeparate(gl.SRC_ALPHA,gl.ONE_MINUS_SRC_ALPHA,gl.ONE,gl.ONE_MINUS_SRC_ALPHA);gl.depthMask(false);gl.bindVertexArray(this.parts[1].vao);gl.drawArrays(gl.TRIANGLES,0,this.parts[1].count);gl.depthMask(true);
    }
  }
  root.MTRenderer=Renderer;
})(globalThis);
