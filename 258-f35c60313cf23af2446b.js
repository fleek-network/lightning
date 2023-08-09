(window.webpackJsonp=window.webpackJsonp||[]).push([[258],{R7lK:function(e,t,n){"use strict";n.r(t),n.d(t,"setupMode",(function(){return E}));var r=function(){function e(e){var t=this;this._defaults=e,this._worker=null,this._idleCheckInterval=setInterval((function(){return t._checkIfIdle()}),3e4),this._lastUsedTime=0,this._configChangeListener=this._defaults.onDidChange((function(){return t._stopWorker()}))}return e.prototype._stopWorker=function(){this._worker&&(this._worker.dispose(),this._worker=null),this._client=null},e.prototype.dispose=function(){clearInterval(this._idleCheckInterval),this._configChangeListener.dispose(),this._stopWorker()},e.prototype._checkIfIdle=function(){this._worker&&(Date.now()-this._lastUsedTime>12e4&&this._stopWorker())},e.prototype._getClient=function(){return this._lastUsedTime=Date.now(),this._client||(this._worker=monaco.editor.createWebWorker({moduleId:"vs/language/json/jsonWorker",label:this._defaults.languageId,createData:{languageSettings:this._defaults.diagnosticsOptions,languageId:this._defaults.languageId,enableSchemaRequest:this._defaults.diagnosticsOptions.enableSchemaRequest}}),this._client=this._worker.getProxy()),this._client},e.prototype.getLanguageServiceWorker=function(){for(var e,t=this,n=[],r=0;r<arguments.length;r++)n[r]=arguments[r];return this._getClient().then((function(t){e=t})).then((function(e){return t._worker.withSyncedResources(n)})).then((function(t){return e}))},e}(),o=n("5peO"),i=(monaco.Uri,monaco.Range),a=function(){function e(e,t,n){var r=this;this._languageId=e,this._worker=t,this._disposables=[],this._listener=Object.create(null);var o=function(e){var t,n=e.getModeId();n===r._languageId&&(r._listener[e.uri.toString()]=e.onDidChangeContent((function(){clearTimeout(t),t=setTimeout((function(){return r._doValidate(e.uri,n)}),500)})),r._doValidate(e.uri,n))},i=function(e){monaco.editor.setModelMarkers(e,r._languageId,[]);var t=e.uri.toString(),n=r._listener[t];n&&(n.dispose(),delete r._listener[t])};this._disposables.push(monaco.editor.onDidCreateModel(o)),this._disposables.push(monaco.editor.onWillDisposeModel((function(e){i(e),r._resetSchema(e.uri)}))),this._disposables.push(monaco.editor.onDidChangeModelLanguage((function(e){i(e.model),o(e.model),r._resetSchema(e.model.uri)}))),this._disposables.push(n.onDidChange((function(e){monaco.editor.getModels().forEach((function(e){e.getModeId()===r._languageId&&(i(e),o(e))}))}))),this._disposables.push({dispose:function(){for(var e in monaco.editor.getModels().forEach(i),r._listener)r._listener[e].dispose()}}),monaco.editor.getModels().forEach(o)}return e.prototype.dispose=function(){this._disposables.forEach((function(e){return e&&e.dispose()})),this._disposables=[]},e.prototype._resetSchema=function(e){this._worker().then((function(t){t.resetSchema(e.toString())}))},e.prototype._doValidate=function(e,t){this._worker(e).then((function(n){return n.doValidation(e.toString()).then((function(n){var r=n.map((function(e){return n="number"==typeof(t=e).code?String(t.code):t.code,{severity:s(t.severity),startLineNumber:t.range.start.line+1,startColumn:t.range.start.character+1,endLineNumber:t.range.end.line+1,endColumn:t.range.end.character+1,message:t.message,code:n,source:t.source};var t,n})),o=monaco.editor.getModel(e);o.getModeId()===t&&monaco.editor.setModelMarkers(o,t,r)}))})).then(void 0,(function(e){console.error(e)}))},e}();function s(e){switch(e){case o.d.Error:return monaco.MarkerSeverity.Error;case o.d.Warning:return monaco.MarkerSeverity.Warning;case o.d.Information:return monaco.MarkerSeverity.Info;case o.d.Hint:return monaco.MarkerSeverity.Hint;default:return monaco.MarkerSeverity.Info}}function u(e){if(e)return{character:e.column-1,line:e.lineNumber-1}}function c(e){if(e)return{start:{line:e.startLineNumber-1,character:e.startColumn-1},end:{line:e.endLineNumber-1,character:e.endColumn-1}}}function l(e){if(e)return new i(e.start.line+1,e.start.character+1,e.end.line+1,e.end.character+1)}function d(e){var t=monaco.languages.CompletionItemKind;switch(e){case o.b.Text:return t.Text;case o.b.Method:return t.Method;case o.b.Function:return t.Function;case o.b.Constructor:return t.Constructor;case o.b.Field:return t.Field;case o.b.Variable:return t.Variable;case o.b.Class:return t.Class;case o.b.Interface:return t.Interface;case o.b.Module:return t.Module;case o.b.Property:return t.Property;case o.b.Unit:return t.Unit;case o.b.Value:return t.Value;case o.b.Enum:return t.Enum;case o.b.Keyword:return t.Keyword;case o.b.Snippet:return t.Snippet;case o.b.Color:return t.Color;case o.b.File:return t.File;case o.b.Reference:return t.Reference}return t.Property}function g(e){if(e)return{range:l(e.range),text:e.newText}}var f=function(){function e(e){this._worker=e}return Object.defineProperty(e.prototype,"triggerCharacters",{get:function(){return[" ",":"]},enumerable:!0,configurable:!0}),e.prototype.provideCompletionItems=function(e,t,n,r){e.getWordUntilPosition(t);var i=e.uri;return this._worker(i).then((function(e){return e.doComplete(i.toString(),u(t))})).then((function(e){if(e){var t=e.items.map((function(e){var t={label:e.label,insertText:e.insertText||e.label,sortText:e.sortText,filterText:e.filterText,documentation:e.documentation,detail:e.detail,kind:d(e.kind)};return e.textEdit&&(t.range=l(e.textEdit.range),t.insertText=e.textEdit.newText),e.additionalTextEdits&&(t.additionalTextEdits=e.additionalTextEdits.map(g)),e.insertTextFormat===o.f.Snippet&&(t.insertTextRules=monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet),t}));return{isIncomplete:e.isIncomplete,suggestions:t}}}))},e}();function h(e){return"string"==typeof e?{value:e}:(t=e)&&"object"==typeof t&&"string"==typeof t.kind?"plaintext"===e.kind?{value:e.value.replace(/[\\`*_{}[\]()#+\-.!]/g,"\\$&")}:{value:e.value}:{value:"```"+e.language+"\n"+e.value+"\n```\n"};var t}function p(e){if(e)return Array.isArray(e)?e.map(h):[h(e)]}var m=function(){function e(e){this._worker=e}return e.prototype.provideHover=function(e,t,n){var r=e.uri;return this._worker(r).then((function(e){return e.doHover(r.toString(),u(t))})).then((function(e){if(e)return{range:l(e.range),contents:p(e.contents)}}))},e}();function v(e){var t=monaco.languages.SymbolKind;switch(e){case o.j.File:return t.Array;case o.j.Module:return t.Module;case o.j.Namespace:return t.Namespace;case o.j.Package:return t.Package;case o.j.Class:return t.Class;case o.j.Method:return t.Method;case o.j.Property:return t.Property;case o.j.Field:return t.Field;case o.j.Constructor:return t.Constructor;case o.j.Enum:return t.Enum;case o.j.Interface:return t.Interface;case o.j.Function:return t.Function;case o.j.Variable:return t.Variable;case o.j.Constant:return t.Constant;case o.j.String:return t.String;case o.j.Number:return t.Number;case o.j.Boolean:return t.Boolean;case o.j.Array:return t.Array}return t.Function}var _=function(){function e(e){this._worker=e}return e.prototype.provideDocumentSymbols=function(e,t){var n=e.uri;return this._worker(n).then((function(e){return e.findDocumentSymbols(n.toString())})).then((function(e){if(e)return e.map((function(e){return{name:e.name,detail:"",containerName:e.containerName,kind:v(e.kind),range:l(e.location.range),selectionRange:l(e.location.range)}}))}))},e}();function k(e){return{tabSize:e.tabSize,insertSpaces:e.insertSpaces}}var b=function(){function e(e){this._worker=e}return e.prototype.provideDocumentFormattingEdits=function(e,t,n){var r=e.uri;return this._worker(r).then((function(e){return e.format(r.toString(),null,k(t)).then((function(e){if(e&&0!==e.length)return e.map(g)}))}))},e}(),w=function(){function e(e){this._worker=e}return e.prototype.provideDocumentRangeFormattingEdits=function(e,t,n,r){var o=e.uri;return this._worker(o).then((function(e){return e.format(o.toString(),c(t),k(n)).then((function(e){if(e&&0!==e.length)return e.map(g)}))}))},e}(),y=function(){function e(e){this._worker=e}return e.prototype.provideDocumentColors=function(e,t){var n=e.uri;return this._worker(n).then((function(e){return e.findDocumentColors(n.toString())})).then((function(e){if(e)return e.map((function(e){return{color:e.color,range:l(e.range)}}))}))},e.prototype.provideColorPresentations=function(e,t,n){var r=e.uri;return this._worker(r).then((function(e){return e.getColorPresentations(r.toString(),t.color,c(t.range))})).then((function(e){if(e)return e.map((function(e){var t={label:e.label};return e.textEdit&&(t.textEdit=g(e.textEdit)),e.additionalTextEdits&&(t.additionalTextEdits=e.additionalTextEdits.map(g)),t}))}))},e}(),C=function(){function e(e){this._worker=e}return e.prototype.provideFoldingRanges=function(e,t,n){var r=e.uri;return this._worker(r).then((function(e){return e.provideFoldingRanges(r.toString(),t)})).then((function(e){if(e)return e.map((function(e){var t={start:e.startLine+1,end:e.endLine+1};return void 0!==e.kind&&(t.kind=function(e){switch(e){case o.e.Comment:return monaco.languages.FoldingRangeKind.Comment;case o.e.Imports:return monaco.languages.FoldingRangeKind.Imports;case o.e.Region:return monaco.languages.FoldingRangeKind.Region}return}(e.kind)),t}))}))},e}();var S=n("wvy9");function I(e){return{getInitialState:function(){return new j(null,null,!1)},tokenize:function(t,n,r,o){return function(e,t,n,r,o){void 0===r&&(r=0);var i=0,a=!1;switch(n.scanError){case 2:t='"'+t,i=1;break;case 1:t="/*"+t,i=2}var s,u,c=S.a(t),l=n.lastWasColon;u={tokens:[],endState:n.clone()};for(;;){var d=r+c.getPosition(),g="";if(17===(s=c.scan()))break;if(d===r+c.getPosition())throw new Error("Scanner did not advance, next 3 characters are: "+t.substr(c.getPosition(),3));switch(a&&(d-=i),a=i>0,s){case 1:case 2:g="delimiter.bracket.json",l=!1;break;case 3:case 4:g="delimiter.array.json",l=!1;break;case 6:g="delimiter.colon.json",l=!0;break;case 5:g="delimiter.comma.json",l=!1;break;case 8:case 9:case 7:g="keyword.json",l=!1;break;case 10:g=l?"string.value.json":"string.key.json",l=!1;break;case 11:g="number.json",l=!1}if(e)switch(s){case 12:g="comment.line.json";break;case 13:g="comment.block.json"}u.endState=new j(n.getStateData(),c.getTokenError(),l),u.tokens.push({startIndex:d,scopes:g})}return u}(e,t,n,r)}}}var j=function(){function e(e,t,n){this._state=e,this.scanError=t,this.lastWasColon=n}return e.prototype.clone=function(){return new e(this._state,this.scanError,this.lastWasColon)},e.prototype.equals=function(t){return t===this||!!(t&&t instanceof e)&&(this.scanError===t.scanError&&this.lastWasColon===t.lastWasColon)},e.prototype.getStateData=function(){return this._state},e.prototype.setStateData=function(e){this._state=e},e}();function E(e){var t=[],n=new r(e);t.push(n);var o=function(){for(var e=[],t=0;t<arguments.length;t++)e[t]=arguments[t];return n.getLanguageServiceWorker.apply(n,e)},i=e.languageId;t.push(monaco.languages.registerCompletionItemProvider(i,new f(o))),t.push(monaco.languages.registerHoverProvider(i,new m(o))),t.push(monaco.languages.registerDocumentSymbolProvider(i,new _(o))),t.push(monaco.languages.registerDocumentFormattingEditProvider(i,new b(o))),t.push(monaco.languages.registerDocumentRangeFormattingEditProvider(i,new w(o))),t.push(new a(i,o,e)),t.push(monaco.languages.setTokensProvider(i,I(!0))),t.push(monaco.languages.setLanguageConfiguration(i,x)),t.push(monaco.languages.registerColorProvider(i,new y(o))),t.push(monaco.languages.registerFoldingRangeProvider(i,new C(o)))}var x={wordPattern:/(-?\d*\.\d\w*)|([^\[\{\]\}\:\"\,\s]+)/g,comments:{lineComment:"//",blockComment:["/*","*/"]},brackets:[["{","}"],["[","]"]],autoClosingPairs:[{open:"{",close:"}",notIn:["string"]},{open:"[",close:"]",notIn:["string"]},{open:'"',close:'"',notIn:["string"]}]}}}]);
//# sourceMappingURL=258-f35c60313cf23af2446b.js.map