const { RawInterpreter } = (() => {

            function setAttributeInner(node,field,value,ns){if(ns==="style"){node.style.setProperty(field,value);return}if(ns){node.setAttributeNS(ns,field,value);return}switch(field){case"value":if(node.tagName==="OPTION")setAttributeDefault(node,field,value);else if(node.value!==value)node.value=value;break;case"initial_value":node.defaultValue=value;break;case"checked":node.checked=truthy(value);break;case"initial_checked":node.defaultChecked=truthy(value);break;case"selected":node.selected=truthy(value);break;case"initial_selected":node.defaultSelected=truthy(value);break;case"dangerous_inner_html":node.innerHTML=value;break;case"style":let existingStyles={};for(let i=0;i<node.style.length;i++){let prop=node.style[i];existingStyles[prop]=node.style.getPropertyValue(prop)}node.setAttribute(field,value);for(let prop in existingStyles)if(!node.style.getPropertyValue(prop))node.style.setProperty(prop,existingStyles[prop]);break;case"multiple":if(setAttributeDefault(node,field,value),node.options!==null&&node.options!==void 0){let options=node.options;for(let option of options)option.selected=option.defaultSelected}break;default:setAttributeDefault(node,field,value)}}function setAttributeDefault(node,field,value){if(!truthy(value)&&isBoolAttr(field))node.removeAttribute(field);else node.setAttribute(field,value)}function truthy(val){return val==="true"||val===!0}function isBoolAttr(field){switch(field){case"allowfullscreen":case"allowpaymentrequest":case"async":case"autofocus":case"autoplay":case"checked":case"controls":case"default":case"defer":case"disabled":case"formnovalidate":case"hidden":case"ismap":case"itemscope":case"loop":case"multiple":case"muted":case"nomodule":case"novalidate":case"open":case"playsinline":case"readonly":case"required":case"reversed":case"selected":case"truespeed":case"webkitdirectory":return!0;default:return!1}}class BaseInterpreter{global;local;root;handler;resizeObserver;intersectionObserver;nodes;stack;templates;m;constructor(){}initialize(root,handler=null){this.global={},this.local={},this.root=root,this.nodes=[root],this.stack=[root],this.templates={},this.handler=handler,root.setAttribute("data-dioxus-id","0")}handleResizeEvent(entry){let target=entry.target,event=new CustomEvent("resize",{bubbles:!1,detail:entry});target.dispatchEvent(event)}createResizeObserver(element){if(!this.resizeObserver)this.resizeObserver=new ResizeObserver((entries)=>{for(let entry of entries)this.handleResizeEvent(entry)});this.resizeObserver.observe(element)}removeResizeObserver(element){if(this.resizeObserver)this.resizeObserver.unobserve(element)}handleIntersectionEvent(entry){let target=entry.target,event=new CustomEvent("visible",{bubbles:!1,detail:entry});target.dispatchEvent(event)}createIntersectionObserver(element){if(!this.intersectionObserver)this.intersectionObserver=new IntersectionObserver((entries)=>{for(let entry of entries)this.handleIntersectionEvent(entry)});this.intersectionObserver.observe(element)}removeIntersectionObserver(element){if(this.intersectionObserver)this.intersectionObserver.unobserve(element)}createListener(event_name,element,bubbles){if(event_name=="resize")this.createResizeObserver(element);else if(event_name=="visible")this.createIntersectionObserver(element);if(bubbles)if(this.global[event_name]===void 0)this.global[event_name]={active:1,callback:this.handler},this.root.addEventListener(event_name,this.handler);else this.global[event_name].active++;else{let id=element.getAttribute("data-dioxus-id");if(!this.local[id])this.local[id]={};element.addEventListener(event_name,this.handler)}}removeListener(element,event_name,bubbles){if(event_name=="resize")this.removeResizeObserver(element);else if(event_name=="visible")this.removeIntersectionObserver(element);else if(bubbles)this.removeBubblingListener(event_name);else this.removeNonBubblingListener(element,event_name)}removeBubblingListener(event_name){if(this.global[event_name].active--,this.global[event_name].active===0)this.root.removeEventListener(event_name,this.global[event_name].callback),delete this.global[event_name]}removeNonBubblingListener(element,event_name){let id=element.getAttribute("data-dioxus-id");if(delete this.local[id][event_name],Object.keys(this.local[id]).length===0)delete this.local[id];element.removeEventListener(event_name,this.handler)}removeAllNonBubblingListeners(element){let id=element.getAttribute("data-dioxus-id");delete this.local[id]}getNode(id){return this.nodes[id]}pushRoot(node){this.stack.push(node)}appendChildren(id,many){let root=this.nodes[id],els=this.stack.splice(this.stack.length-many);for(let k=0;k<many;k++)root.appendChild(els[k])}loadChild(ptr,len){let node=this.stack[this.stack.length-1],ptr_end=ptr+len;for(;ptr<ptr_end;ptr++){let end=this.m.getUint8(ptr);for(node=node.firstChild;end>0;end--)node=node.nextSibling}return node}saveTemplate(nodes,tmpl_id){this.templates[tmpl_id]=nodes}hydrate_node(hydrateNode,ids){let split=hydrateNode.getAttribute("data-node-hydration").split(","),id=ids[parseInt(split[0])];if(this.nodes[id]=hydrateNode,split.length>1){hydrateNode.listening=split.length-1,hydrateNode.setAttribute("data-dioxus-id",id.toString());for(let j=1;j<split.length;j++){let split2=split[j].split(":"),event_name=split2[0],bubbles=split2[1]==="1";this.createListener(event_name,hydrateNode,bubbles)}}}hydrate(ids,underNodes){for(let i=0;i<underNodes.length;i++){let under=underNodes[i];if(under instanceof HTMLElement){if(under.getAttribute("data-node-hydration"))this.hydrate_node(under,ids);let hydrateNodes=under.querySelectorAll("[data-node-hydration]");for(let i2=0;i2<hydrateNodes.length;i2++)this.hydrate_node(hydrateNodes[i2],ids)}let treeWalker=document.createTreeWalker(under,NodeFilter.SHOW_COMMENT),nextSibling=under.nextSibling,continueToNextNode=()=>{if(!treeWalker.nextNode())return!1;return treeWalker.currentNode!==nextSibling};while(treeWalker.currentNode){let currentNode=treeWalker.currentNode;if(currentNode.nodeType===Node.COMMENT_NODE){let id=currentNode.textContent,placeholderSplit=id.split("placeholder");if(placeholderSplit.length>1){if(this.nodes[ids[parseInt(placeholderSplit[1])]]=currentNode,!continueToNextNode())break;continue}let textNodeSplit=id.split("node-id");if(textNodeSplit.length>1){let next=currentNode.nextSibling;currentNode.remove();let commentAfterText,textNode;if(next.nodeType===Node.COMMENT_NODE){let newText=next.parentElement.insertBefore(document.createTextNode(""),next);commentAfterText=next,textNode=newText}else textNode=next,commentAfterText=textNode.nextSibling;treeWalker.currentNode=commentAfterText,this.nodes[ids[parseInt(textNodeSplit[1])]]=textNode;let exit=currentNode===under||!continueToNextNode();if(commentAfterText.remove(),exit)break;continue}}if(!continueToNextNode())break}}}setAttributeInner(node,field,value,ns){setAttributeInner(node,field,value,ns)}}

            let bubbles,field,id,many,ns;
            class RawInterpreter extends BaseInterpreter {
                constructor(r) {
                    super();
                    this.d=r;
                    this.m = null;
                    this.p = null;
                    this.ls = null;
                    this.t = null;
                    this.op = null;
                    this.e = null;
                    this.z = null;
                    this.metaflags = null;
                    this.u16buf=null;this.u16bufp=null;this.u32buf=null;this.u32bufp=null;this.u8buf=null;this.u8bufp=null;this.attr = [];
                    this.attr_cache_hit = null;
                    this.attr_cache_idx;
                    this.get_attr = function() {
                        this.attr_cache_idx = this.u8buf[this.u8bufp++];
                        if(this.attr_cache_idx & 128){
                            this.attr_cache_hit=this.s.substring(this.sp,this.sp+=this.u8buf[this.u8bufp++]);
                            this.attr[this.attr_cache_idx&4294967167]=this.attr_cache_hit;
                            return this.attr_cache_hit;
                        }
                        else{
                            return this.attr[this.attr_cache_idx&4294967167];
                        }
                    };this.el = [];
                    this.el_cache_hit = null;
                    this.el_cache_idx;
                    this.get_el = function() {
                        this.el_cache_idx = this.u8buf[this.u8bufp++];
                        if(this.el_cache_idx & 128){
                            this.el_cache_hit=this.s.substring(this.sp,this.sp+=this.u8buf[this.u8bufp++]);
                            this.el[this.el_cache_idx&4294967167]=this.el_cache_hit;
                            return this.el_cache_hit;
                        }
                        else{
                            return this.el[this.el_cache_idx&4294967167];
                        }
                    };this.evt = [];
                    this.evt_cache_hit = null;
                    this.evt_cache_idx;
                    this.get_evt = function() {
                        this.evt_cache_idx = this.u8buf[this.u8bufp++];
                        if(this.evt_cache_idx & 128){
                            this.evt_cache_hit=this.s.substring(this.sp,this.sp+=this.u8buf[this.u8bufp++]);
                            this.evt[this.evt_cache_idx&4294967167]=this.evt_cache_hit;
                            return this.evt_cache_hit;
                        }
                        else{
                            return this.evt[this.evt_cache_idx&4294967167];
                        }
                    };this.namespace = [];
                    this.namespace_cache_hit = null;
                    this.namespace_cache_idx;
                    this.get_namespace = function() {
                        this.namespace_cache_idx = this.u8buf[this.u8bufp++];
                        if(this.namespace_cache_idx & 128){
                            this.namespace_cache_hit=this.s.substring(this.sp,this.sp+=this.u8buf[this.u8bufp++]);
                            this.namespace[this.namespace_cache_idx&4294967167]=this.namespace_cache_hit;
                            return this.namespace_cache_hit;
                        }
                        else{
                            return this.namespace[this.namespace_cache_idx&4294967167];
                        }
                    };this.ns_cache = [];
                    this.ns_cache_cache_hit = null;
                    this.ns_cache_cache_idx;
                    this.get_ns_cache = function() {
                        this.ns_cache_cache_idx = this.u8buf[this.u8bufp++];
                        if(this.ns_cache_cache_idx & 128){
                            this.ns_cache_cache_hit=this.s.substring(this.sp,this.sp+=this.u8buf[this.u8bufp++]);
                            this.ns_cache[this.ns_cache_cache_idx&4294967167]=this.ns_cache_cache_hit;
                            return this.ns_cache_cache_hit;
                        }
                        else{
                            return this.ns_cache[this.ns_cache_cache_idx&4294967167];
                        }
                    };this.s = "";this.lsp = null;this.sp = null;this.sl = null;this.c = new TextDecoder();
                }

                update_memory(b){
                    this.m=new DataView(b.buffer)
                }

                run(){
                    this.metaflags=this.m.getUint32(this.d,true);
                    if((this.metaflags>>>6)&1){
                        this.ls=this.m.getUint32(this.d+6*4,true);
                    }
                    this.p=this.ls;
                    if ((this.metaflags>>>4)&1){
                this.t = this.m.getUint32(this.d+4*4,true);
                this.u16buf=new Uint16Array(this.m.buffer,this.t,((this.m.buffer.byteLength-this.t)-(this.m.buffer.byteLength-this.t)%2)/2);
            }
            this.u16bufp=0;if ((this.metaflags>>>3)&1){
                this.t = this.m.getUint32(this.d+3*4,true);
                this.u32buf=new Uint32Array(this.m.buffer,this.t,((this.m.buffer.byteLength-this.t)-(this.m.buffer.byteLength-this.t)%4)/4);
            }
            this.u32bufp=0;if ((this.metaflags>>>5)&1){
                this.t = this.m.getUint32(this.d+5*4,true);
                this.u8buf=new Uint8Array(this.m.buffer,this.t,((this.m.buffer.byteLength-this.t)-(this.m.buffer.byteLength-this.t)%1)/1);
            }
            this.u8bufp=0;if (this.metaflags&1){
                this.lsp = this.m.getUint32(this.d+1*4,true);
            }
            if ((this.metaflags>>>2)&1) {
                this.sl = this.m.getUint32(this.d+2*4,true);
                if ((this.metaflags>>>1)&1) {
                    this.sp = this.lsp;
                    this.s = "";
                    this.e = this.sp + ((this.sl / 4) | 0) * 4;
                    while (this.sp < this.e) {
                        this.t = this.m.getUint32(this.sp, true);
                        this.s += String.fromCharCode(
                            this.t & 255,
                            (this.t & 65280) >> 8,
                            (this.t & 16711680) >> 16,
                            this.t >> 24
                        );
                        this.sp += 4;
                    }
                    while (this.sp < this.lsp + this.sl) {
                        this.s += String.fromCharCode(this.m.getUint8(this.sp++));
                    }
                } else {
                    let buffer = new Uint8Array(this.m.buffer, this.lsp, this.sl);


                    if (this.m.buffer.constructor.name === "SharedArrayBuffer") {
                        let arrayBuffer = new ArrayBuffer(this.sl);
                        new Uint8Array(arrayBuffer).set(buffer);
                        buffer = arrayBuffer;
                    }
                    this.s = this.c.decode(buffer);
                }
            }
            this.sp=0;
                    for(;;){
                        this.op=this.m.getUint32(this.p,true);
                        this.p+=4;
                        this.z=0;
                        while(this.z++<4){
                            switch(this.op&255){
                                case 0:{this.pushRoot(this.nodes[this.u32buf[this.u32bufp++]]);}break;case 1:{this.appendChildren(this.u32buf[this.u32bufp++], this.u16buf[this.u16bufp++]);}break;case 2:{this.stack.pop();}break;case 3:{const root = this.nodes[this.u32buf[this.u32bufp++]]; let els = this.stack.splice(this.stack.length-this.u16buf[this.u16bufp++]); if (root.listening) { this.removeAllNonBubblingListeners(root); } root.replaceWith(...els);}break;case 4:{let node = this.nodes[this.u32buf[this.u32bufp++]];node.after(...this.stack.splice(this.stack.length-this.u16buf[this.u16bufp++]));}break;case 5:{let node = this.nodes[this.u32buf[this.u32bufp++]];node.before(...this.stack.splice(this.stack.length-this.u16buf[this.u16bufp++]));}break;case 6:{let node = this.nodes[this.u32buf[this.u32bufp++]]; if (node !== undefined) { if (node.listening) { this.removeAllNonBubblingListeners(node); } node.remove(); }}break;case 7:{this.stack.push(document.createTextNode(this.s.substring(this.sp,this.sp+=this.u32buf[this.u32bufp++])));}break;case 8:{let node = document.createTextNode(this.s.substring(this.sp,this.sp+=this.u32buf[this.u32bufp++])); this.nodes[this.u32buf[this.u32bufp++]] = node; this.stack.push(node);}break;case 9:{let node = document.createComment('placeholder'); this.stack.push(node); this.nodes[this.u32buf[this.u32bufp++]] = node;}break;case 10:id=this.u32buf[this.u32bufp++];
            const node = this.nodes[id];
            if(node.listening){node.listening += 1;}else{node.listening = 1;}
            node.setAttribute('data-dioxus-id', `${id}`);
            this.createListener(this.get_evt(), node, this.u8buf[this.u8bufp++]);
        break;case 11:{let node = this.nodes[this.u32buf[this.u32bufp++]]; node.listening -= 1; node.removeAttribute('data-dioxus-id'); this.removeListener(node, this.get_evt(), this.u8buf[this.u8bufp++]);}break;case 12:{this.nodes[this.u32buf[this.u32bufp++]].textContent = this.s.substring(this.sp,this.sp+=this.u32buf[this.u32bufp++]);}break;case 13:{let node = this.nodes[this.u32buf[this.u32bufp++]]; this.setAttributeInner(node, this.get_attr(), this.s.substring(this.sp,this.sp+=this.u32buf[this.u32bufp++]), this.get_ns_cache());}break;case 14:field=this.get_attr();ns=this.get_ns_cache();{
            let node = this.nodes[this.u32buf[this.u32bufp++]];
            if (!ns) {
                switch (field) {
                    case "value":
                        node.value = "";
                        node.removeAttribute("value");
                        break;
                    case "checked":
                        node.checked = false;
                        break;
                    case "selected":
                        node.selected = false;
                        break;
                    case "dangerous_inner_html":
                        node.innerHTML = "";
                        break;
                    default:
                        node.removeAttribute(field);
                        break;
                }
            } else if (ns == "style") {
                node.style.removeProperty(field);
            } else {
                node.removeAttributeNS(ns, field);
            }
        }break;case 15:{this.nodes[this.u32buf[this.u32bufp++]] = this.loadChild(this.u32buf[this.u32bufp++], this.u8buf[this.u8bufp++]);}break;case 16:{let els = this.stack.splice(this.stack.length - this.u16buf[this.u16bufp++]); let node = this.loadChild(this.u32buf[this.u32bufp++], this.u8buf[this.u8bufp++]); node.replaceWith(...els);}break;case 17:{let node = this.templates[this.u16buf[this.u16bufp++]][this.u16buf[this.u16bufp++]].cloneNode(true); this.nodes[this.u32buf[this.u32bufp++]] = node; this.stack.push(node);}break;case 18:many=this.u16buf[this.u16bufp++];{
            let root = this.stack[this.stack.length-many-1];
            let els = this.stack.splice(this.stack.length-many);
            for (let k = 0; k < many; k++) {
                root.appendChild(els[k]);
            }
        }break;case 19:{this.setAttributeInner(this.stack[this.stack.length-1], this.get_attr(), this.s.substring(this.sp,this.sp+=this.u32buf[this.u32bufp++]), this.get_ns_cache());}break;case 20:{let node = document.createComment('placeholder'); this.stack.push(node);}break;case 21:{this.stack.push(document.createElement(this.get_el()))}break;case 22:{this.stack.push(document.createElementNS(this.get_namespace(), this.get_el()))}break;case 23:{this.templates[this.u16buf[this.u16bufp++]] = this.stack.splice(this.stack.length-this.u16buf[this.u16bufp++]);}break;case 24:id=this.u32buf[this.u32bufp++];bubbles=this.u8buf[this.u8bufp++];
    bubbles = bubbles == 1;
    let this_node = this.nodes[id];
    if(this_node.listening){
        this_node.listening += 1;
    } else {
        this_node.listening = 1;
    }
    this_node.setAttribute('data-dioxus-id', `${id}`);
    const event_name = this.get_evt();


    if (event_name === "mounted") {
        window.ipc.postMessage(
            this.sendSerializedEvent({
                name: event_name,
                element: id,
                data: null,
                bubbles,
            })
        );
    } else {
        this.createListener(event_name, this_node, bubbles, (event) => {
            this.handler(event, event_name, bubbles);
        });
    }break;case 25:{this.nodes[this.u32buf[this.u32bufp++]] = this.loadChild((()=>{this.e=this.u8bufp+this.u32buf[this.u32bufp++];const final_array = this.u8buf.slice(this.u8bufp,this.e);this.u8bufp=this.e;return final_array;})());}break;case 26:{let els = this.stack.splice(this.stack.length - this.u16buf[this.u16bufp++]); let node = this.loadChild((()=>{this.e=this.u8bufp+this.u32buf[this.u32bufp++];const final_array = this.u8buf.slice(this.u8bufp,this.e);this.u8bufp=this.e;return final_array;})()); node.replaceWith(...els);}break;case 27:return true;
                            }
                            this.op>>>=8;
                        }
                    }
                }

                run_from_bytes(bytes){
                    this.d = 0;
                    this.update_memory(new Uint8Array(bytes))
                    this.run()
                }
            }
return { RawInterpreter: RawInterpreter };
})();
const { setAttributeInner } = (() => {
function setAttributeInner(node,field,value,ns){if(ns==="style"){node.style.setProperty(field,value);return}if(ns){node.setAttributeNS(ns,field,value);return}switch(field){case"value":if(node.tagName==="OPTION")setAttributeDefault(node,field,value);else if(node.value!==value)node.value=value;break;case"initial_value":node.defaultValue=value;break;case"checked":node.checked=truthy(value);break;case"initial_checked":node.defaultChecked=truthy(value);break;case"selected":node.selected=truthy(value);break;case"initial_selected":node.defaultSelected=truthy(value);break;case"dangerous_inner_html":node.innerHTML=value;break;case"style":let existingStyles={};for(let i=0;i<node.style.length;i++){let prop=node.style[i];existingStyles[prop]=node.style.getPropertyValue(prop)}node.setAttribute(field,value);for(let prop in existingStyles)if(!node.style.getPropertyValue(prop))node.style.setProperty(prop,existingStyles[prop]);break;case"multiple":if(setAttributeDefault(node,field,value),node.options!==null&&node.options!==void 0){let options=node.options;for(let option of options)option.selected=option.defaultSelected}break;default:setAttributeDefault(node,field,value)}}function setAttributeDefault(node,field,value){if(!truthy(value)&&isBoolAttr(field))node.removeAttribute(field);else node.setAttribute(field,value)}function truthy(val){return val==="true"||val===!0}function isBoolAttr(field){switch(field){case"allowfullscreen":case"allowpaymentrequest":case"async":case"autofocus":case"autoplay":case"checked":case"controls":case"default":case"defer":case"disabled":case"formnovalidate":case"hidden":case"ismap":case"itemscope":case"loop":case"multiple":case"muted":case"nomodule":case"novalidate":case"open":case"playsinline":case"readonly":case"required":case"reversed":case"selected":case"truespeed":case"webkitdirectory":return!0;default:return!1}}

return { setAttributeInner: setAttributeInner };
})();
const { get_select_data } = (() => {

function get_select_data(select) {
    let values = [];
    for (let i = 0; i < select.options.length; i++) {
      let option = select.options[i];
      if (option.selected) {
        values.push(option.value.toString());
      }
    }

    return values;
}

return { get_select_data: get_select_data };
})();
function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg___wbindgen_boolean_get_4f1baf9a00be3c74: function(arg0) {
            const v = arg0;
            const ret = typeof(v) === 'boolean' ? v : undefined;
            return isLikeNone(ret) ? 0xFFFFFF : ret ? 1 : 0;
        },
        __wbg___wbindgen_debug_string_f249a2a76caa9a55: function(arg0, arg1) {
            const ret = debugString(arg1);
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_is_function_741ff2bb612836f5: function(arg0) {
            const ret = typeof(arg0) === 'function';
            return ret;
        },
        __wbg___wbindgen_is_null_c22311b1cc228e3a: function(arg0) {
            const ret = arg0 === null;
            return ret;
        },
        __wbg___wbindgen_is_object_8b968c3ab8e96595: function(arg0) {
            const val = arg0;
            const ret = typeof(val) === 'object' && val !== null;
            return ret;
        },
        __wbg___wbindgen_is_undefined_be5b34249ff1fc15: function(arg0) {
            const ret = arg0 === undefined;
            return ret;
        },
        __wbg___wbindgen_memory_17a1178bc3328ea3: function() {
            const ret = wasm.memory;
            return ret;
        },
        __wbg___wbindgen_number_get_934b640d0889af78: function(arg0, arg1) {
            const obj = arg1;
            const ret = typeof(obj) === 'number' ? obj : undefined;
            getDataViewMemory0().setFloat64((arg0 >>> 0) + 8 * 1, isLikeNone(ret) ? 0 : ret, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, !isLikeNone(ret), true);
        },
        __wbg___wbindgen_string_get_7ce8eee7fd327545: function(arg0, arg1) {
            const obj = arg1;
            const ret = typeof(obj) === 'string' ? obj : undefined;
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg___wbindgen_throw_4c88d8bf264fd984: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbg__wbg_cb_unref_09736879a3f67559: function(arg0) {
            arg0._wbg_cb_unref();
        },
        __wbg_abort_f8d1589eb0c65ee2: function(arg0) {
            arg0.abort();
        },
        __wbg_activeElement_6c30fe7fda33ef51: function(arg0) {
            const ret = arg0.activeElement;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_altKey_423da1d67b588078: function(arg0) {
            const ret = arg0.altKey;
            return ret;
        },
        __wbg_altKey_a82553b3bb99f842: function(arg0) {
            const ret = arg0.altKey;
            return ret;
        },
        __wbg_altKey_f158b37d4bccf5ff: function(arg0) {
            const ret = arg0.altKey;
            return ret;
        },
        __wbg_animationName_00dd5daca275b1ff: function(arg0, arg1) {
            const ret = arg1.animationName;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg_appendChild_dbf7ad2f48413560: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.appendChild(arg1);
            return ret;
        }, arguments); },
        __wbg_append_ba9a4dc30a0323fd: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
            arg0.append(getStringFromWasm0(arg1, arg2), getStringFromWasm0(arg3, arg4));
        }, arguments); },
        __wbg_arrayBuffer_5abbcb61bd48f427: function(arg0) {
            const ret = arg0.arrayBuffer();
            return ret;
        },
        __wbg_blockSize_4a396ef99671a8a3: function(arg0) {
            const ret = arg0.blockSize;
            return ret;
        },
        __wbg_blur_c2856e0bcfec2cc0: function() { return handleError(function (arg0) {
            arg0.blur();
        }, arguments); },
        __wbg_body_0da01a9a056907d8: function(arg0) {
            const ret = arg0.body;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_borderBoxSize_cc2a5649ac55729c: function(arg0) {
            const ret = arg0.borderBoxSize;
            return ret;
        },
        __wbg_bottom_fd840749aac1de6e: function(arg0) {
            const ret = arg0.bottom;
            return ret;
        },
        __wbg_boundingClientRect_9471a8c99d2f7803: function(arg0) {
            const ret = arg0.boundingClientRect;
            return ret;
        },
        __wbg_bubbles_d16d240e1d91bfa3: function(arg0) {
            const ret = arg0.bubbles;
            return ret;
        },
        __wbg_button_653210370515dd71: function(arg0) {
            const ret = arg0.button;
            return ret;
        },
        __wbg_buttons_6c3569a451cd3176: function(arg0) {
            const ret = arg0.buttons;
            return ret;
        },
        __wbg_call_a410eb4b59ed5965: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.call(arg1, arg2);
            return ret;
        }, arguments); },
        __wbg_call_fc92fee6137d452d: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            const ret = arg0.call(arg1, arg2, arg3);
            return ret;
        }, arguments); },
        __wbg_changedTouches_133917dec6f0866e: function(arg0) {
            const ret = arg0.changedTouches;
            return ret;
        },
        __wbg_checkValidity_3ade6cd770a020f0: function(arg0) {
            const ret = arg0.checkValidity();
            return ret;
        },
        __wbg_checked_3f95ede4ece3a2c2: function(arg0) {
            const ret = arg0.checked;
            return ret;
        },
        __wbg_clearData_1ab7b226f3e5da2f: function() { return handleError(function (arg0, arg1, arg2) {
            arg0.clearData(getStringFromWasm0(arg1, arg2));
        }, arguments); },
        __wbg_clearData_b25eb84b43497333: function() { return handleError(function (arg0) {
            arg0.clearData();
        }, arguments); },
        __wbg_clientHeight_b40e8971aed9fa2b: function(arg0) {
            const ret = arg0.clientHeight;
            return ret;
        },
        __wbg_clientWidth_269ea87f17b863bf: function(arg0) {
            const ret = arg0.clientWidth;
            return ret;
        },
        __wbg_clientX_7207eed549b03753: function(arg0) {
            const ret = arg0.clientX;
            return ret;
        },
        __wbg_clientX_8c315b448e031608: function(arg0) {
            const ret = arg0.clientX;
            return ret;
        },
        __wbg_clientY_3371139730cd6f74: function(arg0) {
            const ret = arg0.clientY;
            return ret;
        },
        __wbg_clientY_864eb3eff334b02a: function(arg0) {
            const ret = arg0.clientY;
            return ret;
        },
        __wbg_clipboardData_40289949920202d6: function(arg0) {
            const ret = arg0.clipboardData;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_closest_39824b16065066bc: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.closest(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_code_f0359f3273037430: function(arg0, arg1) {
            const ret = arg1.code;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg_contentBoxSize_d394eb8fa40e0155: function(arg0) {
            const ret = arg0.contentBoxSize;
            return ret;
        },
        __wbg_createComment_bf8f324d5ce3d460: function(arg0, arg1, arg2) {
            const ret = arg0.createComment(getStringFromWasm0(arg1, arg2));
            return ret;
        },
        __wbg_createElementNS_2f7464180077e8e4: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
            const ret = arg0.createElementNS(arg1 === 0 ? undefined : getStringFromWasm0(arg1, arg2), getStringFromWasm0(arg3, arg4));
            return ret;
        }, arguments); },
        __wbg_createElement_de4a31ed2fb252be: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.createElement(getStringFromWasm0(arg1, arg2));
            return ret;
        }, arguments); },
        __wbg_createTextNode_2b48c3fc75dfcc67: function(arg0, arg1, arg2) {
            const ret = arg0.createTextNode(getStringFromWasm0(arg1, arg2));
            return ret;
        },
        __wbg_ctrlKey_6a3cbd9f64f6cd07: function(arg0) {
            const ret = arg0.ctrlKey;
            return ret;
        },
        __wbg_ctrlKey_97b77227d0599549: function(arg0) {
            const ret = arg0.ctrlKey;
            return ret;
        },
        __wbg_ctrlKey_b7978e4cf78dc62b: function(arg0) {
            const ret = arg0.ctrlKey;
            return ret;
        },
        __wbg_dataTransfer_3f478a3e477952f6: function(arg0) {
            const ret = arg0.dataTransfer;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_data_6419bc2d9a3951a4: function(arg0, arg1) {
            const ret = arg1.data;
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg_data_fb9a3ebd8ff4b3fd: function(arg0, arg1) {
            const ret = arg1.data;
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg_deltaMode_d2ba7673c0228446: function(arg0) {
            const ret = arg0.deltaMode;
            return ret;
        },
        __wbg_deltaX_6ad2483f3f4c0349: function(arg0) {
            const ret = arg0.deltaX;
            return ret;
        },
        __wbg_deltaY_37dc0a54748a91f4: function(arg0) {
            const ret = arg0.deltaY;
            return ret;
        },
        __wbg_deltaZ_bea305fbe7ed1569: function(arg0) {
            const ret = arg0.deltaZ;
            return ret;
        },
        __wbg_detail_2174ce1962cc649d: function(arg0) {
            const ret = arg0.detail;
            return ret;
        },
        __wbg_documentElement_5e44e37dd0181a81: function(arg0) {
            const ret = arg0.documentElement;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_document_3b5c3071b43e97ee: function(arg0) {
            const ret = arg0.document;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_done_57e08d0be6e30c02: function(arg0) {
            const ret = arg0.done;
            return ret;
        },
        __wbg_dropEffect_e7e745caa09a9490: function(arg0, arg1) {
            const ret = arg1.dropEffect;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg_effectAllowed_aa1fda9c97e978f1: function(arg0, arg1) {
            const ret = arg1.effectAllowed;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg_elapsedTime_23db3047ce4f2f05: function(arg0) {
            const ret = arg0.elapsedTime;
            return ret;
        },
        __wbg_elapsedTime_3bb8ec0c094e852f: function(arg0) {
            const ret = arg0.elapsedTime;
            return ret;
        },
        __wbg_entries_1174a63a4d7aa1f0: function(arg0) {
            const ret = arg0.entries();
            return ret;
        },
        __wbg_error_d1fc0193610e0e39: function(arg0) {
            console.error(arg0);
        },
        __wbg_fetch_21015a0a4570ab44: function(arg0, arg1, arg2, arg3) {
            const ret = arg0.fetch(getStringFromWasm0(arg1, arg2), arg3);
            return ret;
        },
        __wbg_files_2d8b9138ca028e2c: function(arg0) {
            const ret = arg0.files;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_files_d7e65f35a3acc89f: function(arg0) {
            const ret = arg0.files;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_focus_0a8d513ea5d8338e: function() { return handleError(function (arg0, arg1) {
            arg0.focus(arg1);
        }, arguments); },
        __wbg_focus_d5e2fdbdb56416f0: function() { return handleError(function (arg0) {
            arg0.focus();
        }, arguments); },
        __wbg_force_f08aaeb22f5ab738: function(arg0) {
            const ret = arg0.force;
            return ret;
        },
        __wbg_getAsFile_4f900262b2c5fc57: function() { return handleError(function (arg0) {
            const ret = arg0.getAsFile();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_getAttribute_c7b80d4010284561: function(arg0, arg1, arg2, arg3) {
            const ret = arg1.getAttribute(getStringFromWasm0(arg2, arg3));
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg_getBoundingClientRect_329ca748ec918b0c: function(arg0) {
            const ret = arg0.getBoundingClientRect();
            return ret;
        },
        __wbg_getComputedStyle_3be93eadae0000a9: function() { return handleError(function (arg0, arg1) {
            const ret = arg0.getComputedStyle(arg1);
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_getData_a7ee5645c60062ce: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            const ret = arg1.getData(getStringFromWasm0(arg2, arg3));
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        }, arguments); },
        __wbg_getElementById_7ee213db584f37b0: function(arg0, arg1, arg2) {
            const ret = arg0.getElementById(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_getNode_b14ec18ffa4892a9: function(arg0, arg1) {
            const ret = arg0.getNode(arg1 >>> 0);
            return ret;
        },
        __wbg_getPropertyValue_c0aae8b8341401f1: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            const ret = arg1.getPropertyValue(getStringFromWasm0(arg2, arg3));
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        }, arguments); },
        __wbg_getReader_fadcafe607253a67: function(arg0) {
            const ret = arg0.getReader();
            return ret;
        },
        __wbg_getSelection_ebbf228892c36d69: function() { return handleError(function (arg0) {
            const ret = arg0.getSelection();
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_get_2080c49015e35254: function() { return handleError(function (arg0, arg1) {
            const ret = Reflect.get(arg0, arg1);
            return ret;
        }, arguments); },
        __wbg_get_556f1c9a5c587e8c: function() { return handleError(function (arg0, arg1, arg2, arg3) {
            const ret = arg1.get(getStringFromWasm0(arg2, arg3));
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        }, arguments); },
        __wbg_get_a1361571ec6a08ad: function(arg0, arg1) {
            const ret = arg0[arg1 >>> 0];
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_get_c69af7448e1ad5c0: function(arg0, arg1) {
            const ret = arg0[arg1 >>> 0];
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_get_d7176cfb76d71114: function(arg0, arg1) {
            const ret = arg0[arg1 >>> 0];
            return ret;
        },
        __wbg_get_select_data_bed569d850a0d998: function(arg0, arg1) {
            const ret = get_select_data(arg1);
            const ptr1 = passArrayJsValueToWasm0(ret, wasm.__wbindgen_malloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg_hash_20c2dd23244d8e75: function() { return handleError(function (arg0, arg1) {
            const ret = arg1.hash;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        }, arguments); },
        __wbg_headers_5fc0106c076eb731: function(arg0) {
            const ret = arg0.headers;
            return ret;
        },
        __wbg_height_4e985809fcad22ec: function(arg0) {
            const ret = arg0.height;
            return ret;
        },
        __wbg_height_aae024598db8fec7: function(arg0) {
            const ret = arg0.height;
            return ret;
        },
        __wbg_height_ce5f2b5901ba05ab: function(arg0) {
            const ret = arg0.height;
            return ret;
        },
        __wbg_identifier_a3f04561174d66d3: function(arg0) {
            const ret = arg0.identifier;
            return ret;
        },
        __wbg_initialize_6d1129af221f598b: function(arg0, arg1, arg2) {
            arg0.initialize(arg1, arg2);
        },
        __wbg_inlineSize_0be24bd91656e021: function(arg0) {
            const ret = arg0.inlineSize;
            return ret;
        },
        __wbg_innerWidth_3bc249efddc56481: function() { return handleError(function (arg0) {
            const ret = arg0.innerWidth;
            return ret;
        }, arguments); },
        __wbg_inputType_871ec3c14b5b9f6a: function(arg0, arg1) {
            const ret = arg1.inputType;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg_instanceof_AnimationEvent_a8a73859e95117c9: function(arg0) {
            let result;
            try {
                result = arg0 instanceof AnimationEvent;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_Blob_56431c8f5387c12d: function(arg0) {
            let result;
            try {
                result = arg0 instanceof Blob;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_ClipboardEvent_c4f8f56673b1db0a: function(arg0) {
            let result;
            try {
                result = arg0 instanceof ClipboardEvent;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_CompositionEvent_633208208a4ed6cd: function(arg0) {
            let result;
            try {
                result = arg0 instanceof CompositionEvent;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_CustomEvent_c7fd5d1b316fdfb2: function(arg0) {
            let result;
            try {
                result = arg0 instanceof CustomEvent;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_Document_b3d3a5e7905b4a2f: function(arg0) {
            let result;
            try {
                result = arg0 instanceof Document;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_DragEvent_6f36525d948c6b32: function(arg0) {
            let result;
            try {
                result = arg0 instanceof DragEvent;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_Element_05dbb22ded594343: function(arg0) {
            let result;
            try {
                result = arg0 instanceof Element;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_Event_6a7bcd317c55445f: function(arg0) {
            let result;
            try {
                result = arg0 instanceof Event;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_File_8b277b2875442560: function(arg0) {
            let result;
            try {
                result = arg0 instanceof File;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_FocusEvent_f1bcbb6c7f722d41: function(arg0) {
            let result;
            try {
                result = arg0 instanceof FocusEvent;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_HtmlElement_ea32ea7121e0a496: function(arg0) {
            let result;
            try {
                result = arg0 instanceof HTMLElement;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_HtmlFormElement_d3b1a9ebc7f1a9d3: function(arg0) {
            let result;
            try {
                result = arg0 instanceof HTMLFormElement;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_HtmlInputElement_28751c98d77a5a65: function(arg0) {
            let result;
            try {
                result = arg0 instanceof HTMLInputElement;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_HtmlSelectElement_e8e746c4e56224c9: function(arg0) {
            let result;
            try {
                result = arg0 instanceof HTMLSelectElement;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_HtmlTextAreaElement_37c6efee878106d5: function(arg0) {
            let result;
            try {
                result = arg0 instanceof HTMLTextAreaElement;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_InputEvent_935e9638cd72a0e6: function(arg0) {
            let result;
            try {
                result = arg0 instanceof InputEvent;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_KeyboardEvent_c9280788b471ab5f: function(arg0) {
            let result;
            try {
                result = arg0 instanceof KeyboardEvent;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_MouseEvent_32f858aa00d2ed1d: function(arg0) {
            let result;
            try {
                result = arg0 instanceof MouseEvent;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_Node_01bbf5179851b46b: function(arg0) {
            let result;
            try {
                result = arg0 instanceof Node;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_PointerEvent_cfcc1acd3e7efc24: function(arg0) {
            let result;
            try {
                result = arg0 instanceof PointerEvent;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_ReadableStreamDefaultReader_2fe8bbe95ede87e3: function(arg0) {
            let result;
            try {
                result = arg0 instanceof ReadableStreamDefaultReader;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_Response_5b0b4f7f1daeae51: function(arg0) {
            let result;
            try {
                result = arg0 instanceof Response;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_TouchEvent_0953261a9617faf2: function(arg0) {
            let result;
            try {
                result = arg0 instanceof TouchEvent;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_TransitionEvent_69b47c4821a83752: function(arg0) {
            let result;
            try {
                result = arg0 instanceof TransitionEvent;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_Uint8Array_d7aeb0dcb3108fea: function(arg0) {
            let result;
            try {
                result = arg0 instanceof Uint8Array;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_WheelEvent_22450e5cf332461f: function(arg0) {
            let result;
            try {
                result = arg0 instanceof WheelEvent;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_instanceof_Window_5cf453f5639aa1a2: function(arg0) {
            let result;
            try {
                result = arg0 instanceof Window;
            } catch (_) {
                result = false;
            }
            const ret = result;
            return ret;
        },
        __wbg_intersectionRatio_35708e1cc18263b0: function(arg0) {
            const ret = arg0.intersectionRatio;
            return ret;
        },
        __wbg_intersectionRect_4a08e72a0e64c4a0: function(arg0) {
            const ret = arg0.intersectionRect;
            return ret;
        },
        __wbg_isArray_bcf1376d0d732d08: function(arg0) {
            const ret = Array.isArray(arg0);
            return ret;
        },
        __wbg_isCollapsed_94539f24146e499c: function(arg0) {
            const ret = arg0.isCollapsed;
            return ret;
        },
        __wbg_isComposing_4b9450ae90c4e806: function(arg0) {
            const ret = arg0.isComposing;
            return ret;
        },
        __wbg_isComposing_b1e74068b6ff27e2: function(arg0) {
            const ret = arg0.isComposing;
            return ret;
        },
        __wbg_isIntersecting_ec9e594039aba037: function(arg0) {
            const ret = arg0.isIntersecting;
            return ret;
        },
        __wbg_isPrimary_736ba39364387331: function(arg0) {
            const ret = arg0.isPrimary;
            return ret;
        },
        __wbg_is_2248ff2b575d60c7: function(arg0, arg1) {
            const ret = Object.is(arg0, arg1);
            return ret;
        },
        __wbg_item_5b70fe58b24cf84b: function(arg0, arg1) {
            const ret = arg0.item(arg1 >>> 0);
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_item_ed0dd5477dab9b73: function(arg0, arg1) {
            const ret = arg0.item(arg1 >>> 0);
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_items_37c99e94f307e5a6: function(arg0) {
            const ret = arg0.items;
            return ret;
        },
        __wbg_keyCode_8de649c9821071f4: function(arg0) {
            const ret = arg0.keyCode;
            return ret;
        },
        __wbg_key_76125a528069feff: function(arg0, arg1) {
            const ret = arg1.key;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg_kind_8dfefc415249cc57: function(arg0, arg1) {
            const ret = arg1.kind;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg_lastModified_c103c69e6c705fd3: function(arg0) {
            const ret = arg0.lastModified;
            return ret;
        },
        __wbg_left_9df47701ff317a35: function(arg0) {
            const ret = arg0.left;
            return ret;
        },
        __wbg_length_186163212ffe79c3: function(arg0) {
            const ret = arg0.length;
            return ret;
        },
        __wbg_length_5031527eccbd59fd: function(arg0) {
            const ret = arg0.length;
            return ret;
        },
        __wbg_length_694310723930d77e: function(arg0) {
            const ret = arg0.length;
            return ret;
        },
        __wbg_length_9aab85334c5daf3f: function(arg0) {
            const ret = arg0.length;
            return ret;
        },
        __wbg_length_ee846b4d1e8404a8: function(arg0) {
            const ret = arg0.length;
            return ret;
        },
        __wbg_location_1c5fe5c54f34cfc2: function(arg0) {
            const ret = arg0.location;
            return ret;
        },
        __wbg_location_7320782274fc691b: function(arg0) {
            const ret = arg0.location;
            return ret;
        },
        __wbg_metaKey_33c588e07fd74724: function(arg0) {
            const ret = arg0.metaKey;
            return ret;
        },
        __wbg_metaKey_34b378e7ba352fd6: function(arg0) {
            const ret = arg0.metaKey;
            return ret;
        },
        __wbg_metaKey_5b11de3c1b1edf01: function(arg0) {
            const ret = arg0.metaKey;
            return ret;
        },
        __wbg_name_844773ba0d95a554: function(arg0, arg1) {
            const ret = arg1.name;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg_name_f721590be23206c9: function(arg0, arg1) {
            const ret = arg1.name;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg_new_13de428334042d26: function(arg0) {
            const ret = new Uint8Array(arg0);
            return ret;
        },
        __wbg_new_277c27c26f94cbc2: function() { return handleError(function () {
            const ret = new Headers();
            return ret;
        }, arguments); },
        __wbg_new_5f28518387ad0e34: function() { return handleError(function () {
            const ret = new AbortController();
            return ret;
        }, arguments); },
        __wbg_new_8de01447d206d4a6: function() { return handleError(function () {
            const ret = new DataTransfer();
            return ret;
        }, arguments); },
        __wbg_new_ad1089fcff08268c: function(arg0) {
            const ret = new RawInterpreter(arg0 >>> 0);
            return ret;
        },
        __wbg_new_ba40181b3f24ac80: function(arg0, arg1) {
            try {
                var state0 = {a: arg0, b: arg1};
                var cb0 = (arg0, arg1) => {
                    const a = state0.a;
                    state0.a = 0;
                    try {
                        return wasm_bindgen_cc6ac81a6aa92d46___convert__closures_____invoke___js_sys_34c9ffa1a22b490b___Function_fn_wasm_bindgen_cc6ac81a6aa92d46___JsValue_____wasm_bindgen_cc6ac81a6aa92d46___sys__Undefined___js_sys_34c9ffa1a22b490b___Function_fn_wasm_bindgen_cc6ac81a6aa92d46___JsValue_____wasm_bindgen_cc6ac81a6aa92d46___sys__Undefined_______true_(a, state0.b, arg0, arg1);
                    } finally {
                        state0.a = a;
                    }
                };
                const ret = new Promise(cb0);
                return ret;
            } finally {
                state0.a = 0;
            }
        },
        __wbg_new_cd4348290afae586: function() {
            const ret = new Object();
            return ret;
        },
        __wbg_new_d259f84070c47acb: function() { return handleError(function () {
            const ret = new FileReader();
            return ret;
        }, arguments); },
        __wbg_new_with_form_896a9a625209073a: function() { return handleError(function (arg0) {
            const ret = new FormData(arg0);
            return ret;
        }, arguments); },
        __wbg_next_af5ac6a08bb0a717: function() { return handleError(function (arg0) {
            const ret = arg0.next();
            return ret;
        }, arguments); },
        __wbg_now_043d245551b22f9c: function(arg0) {
            const ret = arg0.now();
            return ret;
        },
        __wbg_offsetX_b3f88f70dc639385: function(arg0) {
            const ret = arg0.offsetX;
            return ret;
        },
        __wbg_offsetY_8037bf2718ef7393: function(arg0) {
            const ret = arg0.offsetY;
            return ret;
        },
        __wbg_ok_670a05555db5f713: function(arg0) {
            const ret = arg0.ok;
            return ret;
        },
        __wbg_ownerDocument_adefd47fd596fe80: function(arg0) {
            const ret = arg0.ownerDocument;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_pageX_71bd040d02ba142a: function(arg0) {
            const ret = arg0.pageX;
            return ret;
        },
        __wbg_pageX_aec68a306094561c: function(arg0) {
            const ret = arg0.pageX;
            return ret;
        },
        __wbg_pageY_b8843b70e0c9e508: function(arg0) {
            const ret = arg0.pageY;
            return ret;
        },
        __wbg_pageY_bbda62d6f85789dd: function(arg0) {
            const ret = arg0.pageY;
            return ret;
        },
        __wbg_parentElement_300cdd39ce1d95f8: function(arg0) {
            const ret = arg0.parentElement;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_performance_50e6a3f8dc80bbca: function(arg0) {
            const ret = arg0.performance;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_pointerId_79742963a7afa662: function(arg0) {
            const ret = arg0.pointerId;
            return ret;
        },
        __wbg_pointerType_c9fa8bdb617406a5: function(arg0, arg1) {
            const ret = arg1.pointerType;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg_pressure_7f8a77640ff8cf66: function(arg0) {
            const ret = arg0.pressure;
            return ret;
        },
        __wbg_preventDefault_cfb06b02e7406899: function(arg0) {
            arg0.preventDefault();
        },
        __wbg_propertyName_681c594b1557cc63: function(arg0, arg1) {
            const ret = arg1.propertyName;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg_prototypesetcall_13162965d1fe7aa3: function(arg0, arg1, arg2) {
            Uint8Array.prototype.set.call(getArrayU8FromWasm0(arg0, arg1), arg2);
        },
        __wbg_pseudoElement_2f7bcc28d5cde940: function(arg0, arg1) {
            const ret = arg1.pseudoElement;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg_pseudoElement_b4fdd3921928c76e: function(arg0, arg1) {
            const ret = arg1.pseudoElement;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg_querySelectorAll_bd785907acdf7800: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.querySelectorAll(getStringFromWasm0(arg1, arg2));
            return ret;
        }, arguments); },
        __wbg_querySelector_72201b7c62dc196b: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.querySelector(getStringFromWasm0(arg1, arg2));
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        }, arguments); },
        __wbg_queueMicrotask_1e8503a8a690f0a3: function(arg0) {
            const ret = arg0.queueMicrotask;
            return ret;
        },
        __wbg_queueMicrotask_4a1a4d8dd332373b: function(arg0) {
            queueMicrotask(arg0);
        },
        __wbg_radiusX_cb10241262205d5a: function(arg0) {
            const ret = arg0.radiusX;
            return ret;
        },
        __wbg_radiusY_4a9f1fd7cbd95443: function(arg0) {
            const ret = arg0.radiusY;
            return ret;
        },
        __wbg_readAsArrayBuffer_4c206b888f53b7da: function() { return handleError(function (arg0, arg1) {
            arg0.readAsArrayBuffer(arg1);
        }, arguments); },
        __wbg_readAsText_1a4367a0b3c18425: function() { return handleError(function (arg0, arg1) {
            arg0.readAsText(arg1);
        }, arguments); },
        __wbg_read_e3bea75dace70752: function(arg0) {
            const ret = arg0.read();
            return ret;
        },
        __wbg_repeat_f86675ce521ba584: function(arg0) {
            const ret = arg0.repeat;
            return ret;
        },
        __wbg_resolve_dbf78ab4942478b0: function(arg0) {
            const ret = Promise.resolve(arg0);
            return ret;
        },
        __wbg_result_18db92cd0d0c3bf5: function() { return handleError(function (arg0) {
            const ret = arg0.result;
            return ret;
        }, arguments); },
        __wbg_rootBounds_c5a72bffac0e00ec: function(arg0) {
            const ret = arg0.rootBounds;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_rotationAngle_e4e7d08609ec537a: function(arg0) {
            const ret = arg0.rotationAngle;
            return ret;
        },
        __wbg_run_c1420b9bde018303: function(arg0) {
            arg0.run();
        },
        __wbg_saveTemplate_e3c1a1e2ac405343: function(arg0, arg1, arg2, arg3) {
            var v0 = getArrayJsValueFromWasm0(arg1, arg2);
            wasm.__wbindgen_free(arg1, arg2 * 4, 4);
            arg0.saveTemplate(v0, arg3);
        },
        __wbg_screenX_414119b35270c86c: function(arg0) {
            const ret = arg0.screenX;
            return ret;
        },
        __wbg_screenX_9891f7ed0600fe26: function(arg0) {
            const ret = arg0.screenX;
            return ret;
        },
        __wbg_screenY_a8dde40da6c35146: function(arg0) {
            const ret = arg0.screenY;
            return ret;
        },
        __wbg_screenY_eedd3e6538a4dbcb: function(arg0) {
            const ret = arg0.screenY;
            return ret;
        },
        __wbg_scrollHeight_879470691b1201b7: function(arg0) {
            const ret = arg0.scrollHeight;
            return ret;
        },
        __wbg_scrollIntoView_2d57abe5fc53ada3: function(arg0, arg1) {
            arg0.scrollIntoView(arg1);
        },
        __wbg_scrollLeft_c7e73c4d493987dd: function(arg0) {
            const ret = arg0.scrollLeft;
            return ret;
        },
        __wbg_scrollTop_23cacc894823db89: function(arg0) {
            const ret = arg0.scrollTop;
            return ret;
        },
        __wbg_scrollWidth_df7cf2f91aa6ab32: function(arg0) {
            const ret = arg0.scrollWidth;
            return ret;
        },
        __wbg_scroll_0ee3d741c5876bbe: function(arg0, arg1) {
            arg0.scroll(arg1);
        },
        __wbg_setAttributeInner_a29bef1cf5d53ffe: function(arg0, arg1, arg2, arg3, arg4, arg5) {
            setAttributeInner(arg0, getStringFromWasm0(arg1, arg2), arg3, arg4 === 0 ? undefined : getStringFromWasm0(arg4, arg5));
        },
        __wbg_setAttribute_f75317b2b5cabfb4: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
            arg0.setAttribute(getStringFromWasm0(arg1, arg2), getStringFromWasm0(arg3, arg4));
        }, arguments); },
        __wbg_setData_ee4b069576146f33: function() { return handleError(function (arg0, arg1, arg2, arg3, arg4) {
            arg0.setData(getStringFromWasm0(arg1, arg2), getStringFromWasm0(arg3, arg4));
        }, arguments); },
        __wbg_setTimeout_9ef22b48643bd7de: function() { return handleError(function (arg0, arg1, arg2) {
            const ret = arg0.setTimeout(arg1, arg2);
            return ret;
        }, arguments); },
        __wbg_set_behavior_162aafb93f17509a: function(arg0, arg1) {
            arg0.behavior = __wbindgen_enum_ScrollBehavior[arg1];
        },
        __wbg_set_behavior_35185ae6de68bf84: function(arg0, arg1) {
            arg0.behavior = __wbindgen_enum_ScrollBehavior[arg1];
        },
        __wbg_set_block_a5fb74a86e30b7a1: function(arg0, arg1) {
            arg0.block = __wbindgen_enum_ScrollLogicalPosition[arg1];
        },
        __wbg_set_body_877d5251e78ca815: function(arg0, arg1) {
            arg0.body = arg1;
        },
        __wbg_set_dropEffect_81bf03e9cf54de98: function(arg0, arg1, arg2) {
            arg0.dropEffect = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_effectAllowed_bc1acaf7b199223f: function(arg0, arg1, arg2) {
            arg0.effectAllowed = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_headers_6d75a7df4eb22716: function(arg0, arg1) {
            arg0.headers = arg1;
        },
        __wbg_set_inline_99055ea8c877c63a: function(arg0, arg1) {
            arg0.inline = __wbindgen_enum_ScrollLogicalPosition[arg1];
        },
        __wbg_set_left_2e185fb3c5a8f843: function(arg0, arg1) {
            arg0.left = arg1;
        },
        __wbg_set_method_639e69af35614d36: function(arg0, arg1, arg2) {
            arg0.method = getStringFromWasm0(arg1, arg2);
        },
        __wbg_set_onload_41d00f0da207135a: function(arg0, arg1) {
            arg0.onload = arg1;
        },
        __wbg_set_prevent_scroll_1d8392770c4be456: function(arg0, arg1) {
            arg0.preventScroll = arg1 !== 0;
        },
        __wbg_set_signal_0b2eee78907e5ba7: function(arg0, arg1) {
            arg0.signal = arg1;
        },
        __wbg_set_top_f7599da231834d9f: function(arg0, arg1) {
            arg0.top = arg1;
        },
        __wbg_set_value_651c5a714f0a7b56: function(arg0, arg1, arg2) {
            arg0.value = getStringFromWasm0(arg1, arg2);
        },
        __wbg_shiftKey_6a84be2563ae8a55: function(arg0) {
            const ret = arg0.shiftKey;
            return ret;
        },
        __wbg_shiftKey_77cf57df5f06bef6: function(arg0) {
            const ret = arg0.shiftKey;
            return ret;
        },
        __wbg_shiftKey_a6ce93ef743d5834: function(arg0) {
            const ret = arg0.shiftKey;
            return ret;
        },
        __wbg_signal_fac908876f45725d: function(arg0) {
            const ret = arg0.signal;
            return ret;
        },
        __wbg_size_9d1084e29743663c: function(arg0) {
            const ret = arg0.size;
            return ret;
        },
        __wbg_static_accessor_GLOBAL_THIS_003ee833af9699fc: function() {
            const ret = typeof globalThis === 'undefined' ? undefined : globalThis;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_GLOBAL_e4293fb22cbe77da: function() {
            const ret = typeof global === 'undefined' ? undefined : global;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_SELF_53bb20457401453d: function() {
            const ret = typeof self === 'undefined' ? undefined : self;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_static_accessor_WINDOW_b5e0810bfecb7feb: function() {
            const ret = typeof window === 'undefined' ? undefined : window;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_status_cb85b6c8d1769fa1: function(arg0) {
            const ret = arg0.status;
            return ret;
        },
        __wbg_tabIndex_411742ceb89b722b: function(arg0) {
            const ret = arg0.tabIndex;
            return ret;
        },
        __wbg_tangentialPressure_44eba53a8c7ad109: function(arg0) {
            const ret = arg0.tangentialPressure;
            return ret;
        },
        __wbg_targetTouches_c20e2e9bc08363af: function(arg0) {
            const ret = arg0.targetTouches;
            return ret;
        },
        __wbg_target_e5cd9c7aa87dd8df: function(arg0) {
            const ret = arg0.target;
            return isLikeNone(ret) ? 0 : addToExternrefTable0(ret);
        },
        __wbg_textContent_240d7f8264a6246e: function(arg0, arg1) {
            const ret = arg1.textContent;
            var ptr1 = isLikeNone(ret) ? 0 : passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg_text_5240eb2c41de8292: function() { return handleError(function (arg0) {
            const ret = arg0.text();
            return ret;
        }, arguments); },
        __wbg_then_656f253a5dd7d8ea: function(arg0, arg1, arg2) {
            const ret = arg0.then(arg1, arg2);
            return ret;
        },
        __wbg_then_e3f7f2d365708be8: function(arg0, arg1) {
            const ret = arg0.then(arg1);
            return ret;
        },
        __wbg_tiltX_3704dfdd7d75e698: function(arg0) {
            const ret = arg0.tiltX;
            return ret;
        },
        __wbg_tiltY_fccfaf59a0909246: function(arg0) {
            const ret = arg0.tiltY;
            return ret;
        },
        __wbg_time_53b3456025b83d5f: function(arg0) {
            const ret = arg0.time;
            return ret;
        },
        __wbg_top_2d45cda4e060d8bf: function(arg0) {
            const ret = arg0.top;
            return ret;
        },
        __wbg_touches_3e4459518c751cbf: function(arg0) {
            const ret = arg0.touches;
            return ret;
        },
        __wbg_twist_da1e62fab894c6f0: function(arg0) {
            const ret = arg0.twist;
            return ret;
        },
        __wbg_type_17ee7b925b1d2343: function(arg0, arg1) {
            const ret = arg1.type;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg_type_5f9ceabba7ef7a1a: function(arg0, arg1) {
            const ret = arg1.type;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg_type_c93f3ee8b7eb4f89: function(arg0, arg1) {
            const ret = arg1.type;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg_update_memory_061ae387dc42f712: function(arg0, arg1) {
            arg0.update_memory(arg1);
        },
        __wbg_value_17829e61bdaa1ac5: function(arg0, arg1) {
            const ret = arg1.value;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg_value_df0a0e29a4d182fe: function(arg0, arg1) {
            const ret = arg1.value;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg_value_e0f328965e184931: function(arg0) {
            const ret = arg0.value;
            return ret;
        },
        __wbg_value_f61036dc298faa36: function(arg0, arg1) {
            const ret = arg1.value;
            const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len1 = WASM_VECTOR_LEN;
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 1, len1, true);
            getDataViewMemory0().setInt32((arg0 >>> 0) + 4 * 0, ptr1, true);
        },
        __wbg_width_7d47cfcf81210264: function(arg0) {
            const ret = arg0.width;
            return ret;
        },
        __wbg_width_b81b030bc60498d5: function(arg0) {
            const ret = arg0.width;
            return ret;
        },
        __wbg_width_d3c8d417d821d2fd: function(arg0) {
            const ret = arg0.width;
            return ret;
        },
        __wbg_x_4e39ec23c8b2fce1: function(arg0) {
            const ret = arg0.x;
            return ret;
        },
        __wbg_y_9a9323be80c4f703: function(arg0) {
            const ret = arg0.y;
            return ret;
        },
        __wbindgen_generic_0000000000000001: function(arg0, arg1) {

            const ret = makeMutClosure(arg0, arg1, wasm_bindgen_cc6ac81a6aa92d46___convert__closures_____invoke___wasm_bindgen_cc6ac81a6aa92d46___JsValue__core_608f92abc48d28da___result__Result_____wasm_bindgen_cc6ac81a6aa92d46___JsError___true_);
            return ret;
        },
        __wbindgen_generic_0000000000000002: function(arg0, arg1) {

            const ret = makeClosure(arg0, arg1, wasm_bindgen_cc6ac81a6aa92d46___convert__closures________invoke___web_sys_9d3df269165f094b___features__gen_Event__Event______true_);
            return ret;
        },
        __wbindgen_generic_0000000000000003: function(arg0, arg1) {

            const ret = makeMutClosure(arg0, arg1, wasm_bindgen_cc6ac81a6aa92d46___convert__closures_____invoke_______true_);
            return ret;
        },
        __wbindgen_generic_0000000000000004: function(arg0, arg1) {

            const ret = getStringFromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_init_externref_table: function() {
            const table = wasm.__wbindgen_externrefs;
            table.set(0, undefined);
            table.set(1024, undefined);
            table.set(1025, null);
            table.set(1026, true);
            table.set(1027, false);
        },
    };
    return {
        __proto__: null,
        "./ansicht_bg.js": import0,
    };
}

function wasm_bindgen_cc6ac81a6aa92d46___convert__closures_____invoke_______true_(arg0, arg1) {
    wasm.wasm_bindgen_cc6ac81a6aa92d46___convert__closures_____invoke_______true_(arg0, arg1);
}

function wasm_bindgen_cc6ac81a6aa92d46___convert__closures________invoke___web_sys_9d3df269165f094b___features__gen_Event__Event______true_(arg0, arg1, arg2) {
    wasm.wasm_bindgen_cc6ac81a6aa92d46___convert__closures________invoke___web_sys_9d3df269165f094b___features__gen_Event__Event______true_(arg0, arg1, arg2);
}

function wasm_bindgen_cc6ac81a6aa92d46___convert__closures_____invoke___wasm_bindgen_cc6ac81a6aa92d46___JsValue__core_608f92abc48d28da___result__Result_____wasm_bindgen_cc6ac81a6aa92d46___JsError___true_(arg0, arg1, arg2) {
    const ret = wasm.wasm_bindgen_cc6ac81a6aa92d46___convert__closures_____invoke___wasm_bindgen_cc6ac81a6aa92d46___JsValue__core_608f92abc48d28da___result__Result_____wasm_bindgen_cc6ac81a6aa92d46___JsError___true_(arg0, arg1, arg2);
    if (ret[1]) {
        throw takeFromExternrefTable0(ret[0]);
    }
}

function wasm_bindgen_cc6ac81a6aa92d46___convert__closures_____invoke___js_sys_34c9ffa1a22b490b___Function_fn_wasm_bindgen_cc6ac81a6aa92d46___JsValue_____wasm_bindgen_cc6ac81a6aa92d46___sys__Undefined___js_sys_34c9ffa1a22b490b___Function_fn_wasm_bindgen_cc6ac81a6aa92d46___JsValue_____wasm_bindgen_cc6ac81a6aa92d46___sys__Undefined_______true_(arg0, arg1, arg2, arg3) {
    wasm.wasm_bindgen_cc6ac81a6aa92d46___convert__closures_____invoke___js_sys_34c9ffa1a22b490b___Function_fn_wasm_bindgen_cc6ac81a6aa92d46___JsValue_____wasm_bindgen_cc6ac81a6aa92d46___sys__Undefined___js_sys_34c9ffa1a22b490b___Function_fn_wasm_bindgen_cc6ac81a6aa92d46___JsValue_____wasm_bindgen_cc6ac81a6aa92d46___sys__Undefined_______true_(arg0, arg1, arg2, arg3);
}


const __wbindgen_enum_ScrollBehavior = ["auto", "instant", "smooth"];


const __wbindgen_enum_ScrollLogicalPosition = ["start", "center", "end", "nearest"];

function addToExternrefTable0(obj) {
    const idx = wasm.__externref_table_alloc();
    wasm.__wbindgen_externrefs.set(idx, obj);
    return idx;
}

const CLOSURE_DTORS = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(state => wasm.__wbindgen_destroy_closure(state.a, state.b));

function debugString(val) {
    switch (typeof val) {

        case 'number':
        case 'boolean':
        case 'undefined':
        return `${val}`;
        case 'string':
        return JSON.stringify(val);
        case 'bigint':
        return `${val}n`;
        case 'symbol':
        return val.description !== undefined
            ? `Symbol(${val.description})`
            : 'Symbol';
        case 'function':
        return typeof val.name === 'string' && val.name !== ''
            ? `Function(${val.name})`
            : 'Function';
        case 'object': {
            if (val === null) {
                return 'null';
            }

            if (Array.isArray(val)) {
                return '[' + Array.from(val, debugString).join(', ') + ']';
            }

            if (val instanceof Error) {
                return `${val.name}: ${val.message}\n${val.stack}`;
            }

            const toStringOutput = toString.call(val);

            const className = /\[object ([^\]]+)\]/.exec(toStringOutput)?.[1];


            if (className === undefined) {
                return toStringOutput;
            }
            if (className === 'Object') {



                try {
                    return 'Object(' + JSON.stringify(val) + ')';
                } catch (_) {
                    return 'Object';
                }
            }

            return className;
        }


    }
}

function getArrayJsValueFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    const mem = getDataViewMemory0();
    const result = [];
    for (let i = ptr; i < ptr + 4 * len; i += 4) {
        result.push(wasm.__wbindgen_externrefs.get(mem.getUint32(i, true)));
    }
    wasm.__externref_drop_slice(ptr, len);
    return result;
}

function getArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

let cachedDataViewMemory0 = null;
function getDataViewMemory0() {
    if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
        cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
    }
    return cachedDataViewMemory0;
}

function getStringFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return decodeText(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function handleError(f, args) {
    try {
        return f.apply(this, args);
    } catch (e) {
        const idx = addToExternrefTable0(e);
        wasm.__wbindgen_exn_store(idx);
    }
}

function isLikeNone(x) {
    return x === undefined || x === null;
}

function makeClosure(arg0, arg1, f) {
    const state = { a: arg0, b: arg1, cnt: 1 };
    const real = (...args) => {




        state.cnt++;
        try {
            return f(state.a, state.b, ...args);
        } finally {
            real._wbg_cb_unref();
        }
    };
    real._wbg_cb_unref = () => {
        if (--state.cnt === 0) {
            wasm.__wbindgen_destroy_closure(state.a, state.b);
            state.a = 0;
            CLOSURE_DTORS.unregister(state);
        }
    };
    CLOSURE_DTORS.register(real, state, state);
    return real;
}

function makeMutClosure(arg0, arg1, f) {
    const state = { a: arg0, b: arg1, cnt: 1 };
    const real = (...args) => {




        state.cnt++;
        const a = state.a;
        state.a = 0;
        try {
            return f(a, state.b, ...args);
        } finally {
            state.a = a;
            real._wbg_cb_unref();
        }
    };
    real._wbg_cb_unref = () => {
        if (--state.cnt === 0) {
            wasm.__wbindgen_destroy_closure(state.a, state.b);
            state.a = 0;
            CLOSURE_DTORS.unregister(state);
        }
    };
    CLOSURE_DTORS.register(real, state, state);
    return real;
}

function passArrayJsValueToWasm0(array, malloc) {
    const ptr = malloc(array.length * 4, 4) >>> 0;
    for (let i = 0; i < array.length; i++) {
        const add = addToExternrefTable0(array[i]);
        getDataViewMemory0().setUint32(ptr + 4 * i, add, true);
    }
    WASM_VECTOR_LEN = array.length;
    return ptr;
}

function passStringToWasm0(arg, malloc, realloc) {
    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

function takeFromExternrefTable0(idx) {
    const value = wasm.__wbindgen_externrefs.get(idx);
    wasm.__externref_table_dealloc(idx);
    return value;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(bytes) {
    numBytesDecoded += bytes.length;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true });
        cachedTextDecoder.decode();
        numBytesDecoded = bytes.length;
    }
    return cachedTextDecoder.decode(bytes);
}

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    };
}

let WASM_VECTOR_LEN = 0;

let wasmModule, wasmInstance, wasm;
function __wbg_finalize_init(instance, module) {
    wasmInstance = instance;
    wasm = instance.exports;
    wasmModule = module;
    cachedDataViewMemory0 = null;
    cachedUint8ArrayMemory0 = null;
    wasm.__wbindgen_start();
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (!module.ok) {
            throw new Error(`failed to fetch Wasm: ${module.status} ${module.statusText} fetching '${module.url}'`);
        }

        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = expectedResponseType(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else { throw e; }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }

    function expectedResponseType(type) {
        switch (type) {
            case 'basic': case 'cors': case 'default': return true;
        }
        return false;
    }
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (module !== undefined) {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

let __wbg_init_promise;
async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;
    if (__wbg_init_promise === undefined) {
        __wbg_init_promise = __wbg_init_once(module_or_path);
        __wbg_init_promise.catch(() => { __wbg_init_promise = undefined; });
    }
    return __wbg_init_promise;
}

async function __wbg_init_once(module_or_path) {

    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('ansicht_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };

__wbg_init();
