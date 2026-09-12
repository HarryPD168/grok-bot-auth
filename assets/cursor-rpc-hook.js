;(function () {
  if (globalThis.__gb_rpc_redirect) return;
  globalThis.__gb_rpc_redirect = true;
  var LOCAL = "http://127.0.0.1:47821";
  var LOCAL_HOST = "127.0.0.1";
  var LOCAL_PORT = 47821;
  var HIT = [
    "AvailableModels",
    "GetUsableModels",
    "BidiAppend",
    "RunSSE",
    "agent.v1.AgentService/Run",
    "GetNewChatNudgeParameterizedModelPicker",
    "GetPromptContextUsage",
    "UploadConversationBlobs",
    "NotifyConversationClone",
    "NameAgent",
    "GetSignedUrlForAttachedMedia",
    "/agent/v1/run"
  ];
  var CATALOG = [
    "AvailableModels",
    "GetUsableModels",
    "GetNewChatNudgeParameterizedModelPicker"
  ];
  var localUp = false;
  var probing = false;
  var injectUntil = 0;
  var heldSse = null;
  var localIds = {};
  var LOCAL_ID_TTL = 10 * 60 * 1000;
  function hitPath(p) {
    p = String(p || "");
    if (p.indexOf("RunPoll") >= 0) return false;
    for (var i = 0; i < HIT.length; i++) {
      if (p.indexOf(HIT[i]) >= 0) return true;
    }
    return false;
  }
  function isCatalog(p) {
    p = String(p || "");
    for (var i = 0; i < CATALOG.length; i++) {
      if (p.indexOf(CATALOG[i]) >= 0) return true;
    }
    return false;
  }
  function isAgent(p) {
    p = String(p || "");
    return (
      p.indexOf("BidiAppend") >= 0 ||
      p.indexOf("RunSSE") >= 0 ||
      p.indexOf("agent.v1.AgentService/Run") >= 0 ||
      p.indexOf("GetPromptContextUsage") >= 0 ||
      p.indexOf("UploadConversationBlobs") >= 0 ||
      p.indexOf("NotifyConversationClone") >= 0 ||
      p.indexOf("NameAgent") >= 0 ||
      p.indexOf("GetSignedUrlForAttachedMedia") >= 0 ||
      p.indexOf("/agent/v1/run") >= 0
    );
  }
  function isRunSse(p) {
    return String(p || "").indexOf("RunSSE") >= 0;
  }
  function isBidiAppend(p) {
    return String(p || "").indexOf("BidiAppend") >= 0;
  }
  function looksInjected(buf) {
    if (!buf || !buf.length) return false;
    var s = Buffer.isBuffer(buf) ? buf.toString("latin1") : String(buf);
    if (s.indexOf("gb-") >= 0) return true;
    if (s.toLowerCase().indexOf("67622d") >= 0) return true;
    return false;
  }
  function extractReqId(buf) {
    if (!buf || !buf.length) return "";
    var s = Buffer.isBuffer(buf) ? buf.toString("latin1") : String(buf);
    var m = s.match(/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/i);
    return m ? m[0] : "";
  }
  function rememberLocal(id) {
    if (!id) return;
    localIds[id] = Date.now() + LOCAL_ID_TTL;
  }
  function isRemembered(id) {
    if (!id) return false;
    var exp = localIds[id];
    return !!(exp && Date.now() < exp);
  }
  function allUuids(body) {
    var s = Buffer.isBuffer(body) ? body.toString("latin1") : String(body);
    var re = /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/gi;
    var out = [];
    var m;
    while ((m = re.exec(s))) out.push(m[0]);
    return out;
  }
  function anyRemembered(body) {
    var ids = allUuids(body);
    for (var i = 0; i < ids.length; i++) {
      if (isRemembered(ids[i])) return true;
    }
    return false;
  }
  function hasLiveLocal() {
    var now = Date.now();
    for (var id in localIds) {
      if (localIds[id] && now < localIds[id]) return true;
    }
    return false;
  }
  function isStickyUnary(p) {
    p = String(p || "");
    return (
      p.indexOf("GetPromptContextUsage") >= 0 ||
      p.indexOf("UploadConversationBlobs") >= 0 ||
      p.indexOf("NotifyConversationClone") >= 0 ||
      p.indexOf("NameAgent") >= 0 ||
      p.indexOf("GetSignedUrlForAttachedMedia") >= 0
    );
  }
  function stickLocal(body, path) {
    var id = extractReqId(body);
    var injected = looksInjected(body);
    if (injected) {
      rememberLocal(id);
      var ids = allUuids(body);
      for (var i = 0; i < ids.length; i++) rememberLocal(ids[i]);
      injectUntil = Date.now() + 30000;
    }
    if (isStickyUnary(path)) {
      var p = String(path || "");
      if (p.indexOf("NameAgent") >= 0) {
        return injected || anyRemembered(body) || (hasLiveLocal() && Date.now() < injectUntil + 90000);
      }
      if (p.indexOf("UploadConversationBlobs") >= 0) {
        return injected || anyRemembered(body) || Date.now() < injectUntil;
      }
      return injected || anyRemembered(body);
    }
    return injected || isRemembered(id);
  }
  function probeLocal() {
    if (probing) return;
    probing = true;
    try {
      var net = require("net");
      var sock = net.connect({ host: LOCAL_HOST, port: LOCAL_PORT });
      sock.setTimeout(80);
      function done(up) {
        localUp = !!up;
        probing = false;
        try {
          sock.destroy();
        } catch (e) {}
      }
      sock.once("connect", function () {
        done(true);
      });
      sock.once("timeout", function () {
        done(false);
      });
      sock.once("error", function () {
        done(false);
      });
    } catch (e) {
      localUp = false;
      probing = false;
    }
  }
  probeLocal();
  try {
    setInterval(probeLocal, 1500);
  } catch (e) {}
  function normHost(host) {
    return String(host || "")
      .trim()
      .replace(/^\[|\]$/g, "")
      .toLowerCase();
  }
  function isLocal(host, port) {
    host = normHost(host);
    if (host !== "127.0.0.1" && host !== "localhost") return false;
    return !port || String(port) === String(LOCAL_PORT);
  }
  function isCursorApi(host) {
    host = normHost(host);
    return /(^|\.)api[2345]\.cursor\.sh$|(^|\.)gcpp\.cursor\.sh$|^api\.playground\.cursor\.sh$/.test(
      host
    );
  }
  function pathOnly(p) {
    p = String(p || "");
    var q = p.indexOf("?");
    return q === -1 ? p : p.slice(0, q);
  }
  function parseInput(input, options) {
    try {
      if (typeof input === "string") {
        var u = new URL(input);
        return {
          hostname: u.hostname,
          port: u.port,
          path: u.pathname + u.search,
          protocol: u.protocol
        };
      }
      if (input && typeof input === "object" && input.href && !input.path && !input.hostname) {
        var u2 = new URL(input.href);
        return {
          hostname: u2.hostname,
          port: u2.port,
          path: u2.pathname + u2.search,
          protocol: u2.protocol
        };
      }
      var src = input && typeof input === "object" ? input : options || {};
      var hostText = String(src.hostname || src.host || "");
      var port = src.port || "";
      if (!src.hostname && hostText.indexOf(":") > 0) {
        var colon = hostText.lastIndexOf(":");
        if (/^\d+$/.test(hostText.slice(colon + 1))) {
          if (!port) port = hostText.slice(colon + 1);
          hostText = hostText.slice(0, colon);
        }
      }
      return {
        hostname: hostText,
        port: String(port || ""),
        path: src.path || (src.pathname || "/") + (src.search || ""),
        protocol: src.protocol || "",
        headers: src.headers
      };
    } catch (e) {
      return null;
    }
  }
  function copyHeaders(headers) {
    var out = {};
    if (!headers) return out;
    if (typeof headers.forEach === "function") {
      try {
        headers.forEach(function (value, key) {
          if (String(key).toLowerCase() === "host") return;
          out[key] = value;
        });
        return out;
      } catch (e) {}
    }
    if (Array.isArray(headers)) {
      if (headers.length && Array.isArray(headers[0])) {
        for (var i = 0; i < headers.length; i++) {
          var k = String(headers[i][0] || "");
          if (!k || k.charAt(0) === ":" || k.toLowerCase() === "host") continue;
          out[k] = headers[i][1];
        }
        return out;
      }
      for (var j = 0; j + 1 < headers.length; j += 2) {
        var key = String(headers[j] || "");
        if (!key || key.charAt(0) === ":" || key.toLowerCase() === "host") continue;
        out[key] = headers[j + 1];
      }
      return out;
    }
    Object.keys(headers).forEach(function (key) {
      if (!key || key.charAt(0) === ":" || String(key).toLowerCase() === "host") return;
      out[key] = headers[key];
    });
    return out;
  }
  function pathFromH2(headers) {
    if (!headers) return "";
    if (Array.isArray(headers)) {
      if (headers.length && Array.isArray(headers[0])) {
        for (var i = 0; i < headers.length; i++) {
          if (String(headers[i][0]).toLowerCase() === ":path")
            return String(headers[i][1] || "");
        }
      } else {
        for (var j = 0; j + 1 < headers.length; j += 2) {
          if (String(headers[j]).toLowerCase() === ":path")
            return String(headers[j + 1] || "");
        }
      }
      return "";
    }
    return String(headers[":path"] || headers[":PATH"] || "");
  }
  try {
    var http = require("http");
    var https = require("https");
    var nodeModule = require("module");
    var origHttpReq = http.request;
    var origHttpsReq = https.request;
    var origHttpGet = http.get;
    var origHttpsGet = https.get;
    var vscodeHttp = http.__vscodeOriginal;
    var directReq = (vscodeHttp && vscodeHttp.request) || origHttpReq;
    function localRequest(path, headers, cb) {
      return directReq.call(http, {
        protocol: "http:",
        hostname: LOCAL_HOST,
        port: LOCAL_PORT,
        path: pathOnly(path) || "/",
        method: "POST",
        headers: copyHeaders(headers)
      }, cb);
    }
    function catalogKind(path) {
      var p = String(path || "");
      if (p.indexOf("GetUsableModels") >= 0) return "usable";
      if (p.indexOf("Nudge") >= 0 || p.indexOf("nudge") >= 0) return "nudge";
      return "available";
    }
    function postMerge(kind, contentType, body, cb) {
      var done = false;
      function finish(err, buf) {
        if (done) return;
        done = true;
        try {
          cb(err, buf);
        } catch (e) {}
      }
      var timer = setTimeout(function () {
        finish(new Error("merge timeout"));
      }, 2000);
      var payload = Buffer.isBuffer(body) ? body : Buffer.from(body || []);
      var req = directReq.call(
        http,
        {
          protocol: "http:",
          hostname: LOCAL_HOST,
          port: LOCAL_PORT,
          path: "/relay/merge-catalog",
          method: "POST",
          headers: {
            "content-type": contentType || "application/proto",
            "content-length": String(payload.length),
            "x-gb-catalog": kind || "available"
          }
        },
        function (res) {
          var chunks = [];
          res.on("data", function (c) {
            chunks.push(Buffer.isBuffer(c) ? c : Buffer.from(c));
          });
          res.on("end", function () {
            clearTimeout(timer);
            finish(null, Buffer.concat(chunks));
          });
        }
      );
      req.on("error", function (err) {
        clearTimeout(timer);
        finish(err);
      });
      req.end(payload);
    }
    function wrapCatalogMerge(req, path) {
      var origOn = req.on.bind(req);
      var origOnce = req.once.bind(req);
      var respFns = [];
      var hooked = false;
      function interceptResponse(fn) {
        if (typeof fn === "function") respFns.push(fn);
        if (hooked) return;
        hooked = true;
        origOn("response", function (res) {
          var chunks = [];
          res.on("data", function (c) {
            chunks.push(Buffer.isBuffer(c) ? c : Buffer.from(c));
          });
          res.on("error", function () {
            for (var i = 0; i < respFns.length; i++) {
              try {
                respFns[i](res);
              } catch (e) {}
            }
          });
          res.on("end", function () {
            var body = Buffer.concat(chunks);
            var ct =
              (res.headers && (res.headers["content-type"] || res.headers["Content-Type"])) ||
              "application/proto";
            if (Array.isArray(ct)) ct = ct[0];
            function emitFake(buf) {
              var Readable = require("stream").Readable;
              var fake = new Readable({
                read: function () {}
              });
              fake.statusCode = res.statusCode || 200;
              fake.headers = Object.assign({}, res.headers || {});
              fake.headers["content-length"] = String(buf.length);
              for (var i = 0; i < respFns.length; i++) {
                try {
                  respFns[i](fake);
                } catch (e) {}
              }
              process.nextTick(function () {
                fake.push(buf);
                fake.push(null);
              });
            }
            if (!localUp) {
              emitFake(body);
              return;
            }
            postMerge(catalogKind(path), ct, body, function (err, merged) {
              emitFake(!err && merged && merged.length ? merged : body);
            });
          });
        });
      }
      req.on = function (ev, fn) {
        if (ev === "response") {
          interceptResponse(fn);
          return req;
        }
        return origOn(ev, fn);
      };
      req.once = function (ev, fn) {
        if (ev === "response") {
          interceptResponse(fn);
          return req;
        }
        return origOnce(ev, fn);
      };
      return req;
    }
    function wrapH2CatalogMerge(official, path) {
      var chunks = [];
      var ended = false;
      var ct = "application/proto";
      var origEmit = official.emit;
      official.emit = function (ev) {
        if (ev === "response") {
          var hdrs = arguments[1];
          if (hdrs) {
            var raw = hdrs["content-type"] || hdrs["Content-Type"];
            if (Array.isArray(raw)) raw = raw[0];
            if (raw) ct = raw;
          }
          return origEmit.apply(this, arguments);
        }
        if (ev === "data") {
          var c = arguments[1];
          chunks.push(Buffer.isBuffer(c) ? c : Buffer.from(c || []));
          return false;
        }
        if (ev === "end") {
          if (ended) return origEmit.apply(this, arguments);
          ended = true;
          var body = Buffer.concat(chunks);
          var self = this;
          var endArgs = arguments;
          function finish(buf) {
            origEmit.call(self, "data", buf);
            origEmit.apply(self, endArgs);
          }
          if (!localUp) {
            finish(body);
            return false;
          }
          postMerge(catalogKind(path), ct, body, function (err, merged) {
            finish(!err && merged && merged.length ? merged : body);
          });
          return false;
        }
        return origEmit.apply(this, arguments);
      };
      return official;
    }
    function officialRequest(isHttps, args) {
      return (isHttps ? origHttpsReq : origHttpReq).apply(isHttps ? https : http, args);
    }
    function pipeFacade(facade, req) {
      facade._inner = req;
      var evs = facade._evs || [];
      for (var i = 0; i < evs.length; i++) {
        try {
          req.on(evs[i][0], evs[i][1]);
        } catch (e) {}
      }
      facade._evs = [];
      var writes = facade._buf || [];
      for (var j = 0; j < writes.length; j++) {
        try {
          req.write(writes[j]);
        } catch (e) {}
      }
      facade._buf = [];
      if (facade._ended) {
        try {
          req.end();
        } catch (e) {}
      }
      return req;
    }
    function makeFacade() {
      var facade = {
        _inner: null,
        _buf: [],
        _evs: [],
        _ended: false,
        write: function (chunk, enc, cb) {
          if (this._inner) return this._inner.write(chunk, enc, cb);
          if (chunk) this._buf.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk, enc));
          if (typeof enc === "function") enc();
          else if (typeof cb === "function") cb();
          return true;
        },
        end: function (chunk, enc, cb) {
          if (chunk) this.write(chunk, enc);
          this._ended = true;
          if (typeof this._onend === "function") this._onend();
          if (this._inner) return this._inner.end();
          if (typeof enc === "function") enc();
          else if (typeof cb === "function") cb();
        },
        on: function (ev, fn) {
          if (this._inner) return this._inner.on(ev, fn);
          this._evs.push([ev, fn]);
          return this;
        },
        once: function (ev, fn) {
          if (this._inner) return this._inner.once(ev, fn);
          this._evs.push([ev, fn]);
          return this;
        },
        abort: function () {
          if (this._inner && this._inner.abort) this._inner.abort();
        },
        destroy: function (err) {
          if (this._inner && this._inner.destroy) this._inner.destroy(err);
        },
        setTimeout: function () {
          if (this._inner && this._inner.setTimeout)
            return this._inner.setTimeout.apply(this._inner, arguments);
          return this;
        },
        setHeader: function () {},
        getHeader: function () {},
        removeHeader: function () {},
        flushHeaders: function () {}
      };
      return facade;
    }
    function releaseHeld(toLocal, reqId) {
      if (!heldSse) return;
      if (heldSse.reqId && reqId && heldSse.reqId !== reqId) return;
      var held = heldSse;
      heldSse = null;
      try {
        clearTimeout(held.timer);
      } catch (e) {}
      try {
        (toLocal ? held.sendLocal : held.sendOfficial)();
      } catch (e) {}
    }
    function intercept(isHttps) {
      return function (input, options, cb) {
        try {
          var parsed = parseInput(input, options);
          if (parsed && isLocal(parsed.hostname, parsed.port)) {
            return directReq.apply(http, arguments);
          }
          var args = arguments;
          if (
            parsed &&
            isCursorApi(parsed.hostname) &&
            hitPath(parsed.path)
          ) {
            var callback = typeof options === "function" ? options : cb;
            var hdrs =
              (input && typeof input === "object" && input.headers) ||
              (options && typeof options === "object" && options.headers) ||
              parsed.headers;
            if (!localUp) {
              return officialRequest(isHttps, args);
            }
            if (isCatalog(parsed.path)) {
              var catArgs = Array.prototype.slice.call(args);
              var catCb = null;
              if (typeof catArgs[catArgs.length - 1] === "function") {
                catCb = catArgs.pop();
              }
              var officialCat = (isHttps ? origHttpsReq : origHttpReq).apply(
                isHttps ? https : http,
                catArgs
              );
              wrapCatalogMerge(officialCat, parsed.path);
              if (catCb) officialCat.on("response", catCb);
              officialCat.on("error", function () {
                probeLocal();
              });
              return officialCat;
            }
            if (isAgent(parsed.path)) {
              var facade = makeFacade();
              var sent = false;
              function sendLocal() {
                if (sent) return;
                sent = true;
                pipeFacade(facade, localRequest(parsed.path, hdrs, callback));
              }
              function sendOfficial() {
                if (sent) return;
                sent = true;
                pipeFacade(facade, officialRequest(isHttps, args));
              }
              facade._onend = function () {
                var body = Buffer.concat(facade._buf || []);
                var toLocal = stickLocal(body, parsed.path);
                var reqId = extractReqId(body);
                if (isRunSse(parsed.path)) {
                  if (toLocal) {
                    sendLocal();
                    return;
                  }
                  releaseHeld(false, reqId);
                  heldSse = {
                    reqId: reqId,
                    sendLocal: sendLocal,
                    sendOfficial: sendOfficial,
                    timer: setTimeout(function () {
                      if (heldSse && heldSse.sendOfficial === sendOfficial) {
                        heldSse = null;
                        sendOfficial();
                      }
                    }, 2000)
                  };
                  return;
                }
                if (isBidiAppend(parsed.path)) {
                  releaseHeld(toLocal, reqId);
                  if (toLocal) sendLocal();
                  else sendOfficial();
                  return;
                }
                if (toLocal) sendLocal();
                else sendOfficial();
              };
              return facade;
            }
          }
        } catch (e) {}
        return (isHttps ? origHttpsReq : origHttpReq).apply(this, arguments);
      };
    }
    function interceptGet(isHttps) {
      return function () {
        var req = intercept(isHttps).apply(this, arguments);
        try {
          req.end();
        } catch (e) {}
        return req;
      };
    }
    http.request = intercept(false);
    https.request = intercept(true);
    http.get = interceptGet(false);
    https.get = interceptGet(true);
    try {
      if (typeof nodeModule.syncBuiltinESMExports === "function")
        nodeModule.syncBuiltinESMExports();
    } catch (e) {}
    function divertH1(path, headers) {
      var Duplex = require("stream").Duplex;
      var ended = false;
      var idle;
      var h1req = localRequest(path, headers);
      var stream = new Duplex({
        write: function (chunk, enc, cb) {
          if (!ended) {
            h1req.write(chunk, enc);
            clearTimeout(idle);
            idle = setTimeout(function () {
              if (ended) return;
              ended = true;
              try {
                h1req.end();
              } catch (e) {}
            }, 400);
          }
          cb();
        },
        final: function (cb) {
          clearTimeout(idle);
          if (!ended) {
            ended = true;
            try {
              h1req.end();
            } catch (e) {}
          }
          cb();
        },
        read: function () {}
      });
      h1req.on("response", function (res) {
        var hdrs = { ":status": String(res.statusCode || 200) };
        Object.keys(res.headers || {}).forEach(function (key) {
          hdrs[key] = res.headers[key];
        });
        stream.emit("response", hdrs);
        res.on("data", function (c) {
          stream.push(c);
        });
        res.on("end", function () {
          stream.emit("trailers", {});
          stream.push(null);
        });
      });
      h1req.on("error", function (err) {
        stream.destroy(err);
      });
      stream._destroy = function (err, cb) {
        try {
          h1req.destroy();
        } catch (e) {}
        cb(err);
      };
      return stream;
    }
    try {
      var http2 = require("http2");
      if (http2 && !http2.__gb_wrapped) {
        http2.__gb_wrapped = true;
        var origConnect = http2.connect;
        http2.connect = function (authority) {
          try {
            var host = "";
            if (typeof authority === "string") {
              try {
                host = new URL(authority).hostname;
              } catch (e) {
                host = String(authority);
              }
            } else if (authority && authority.hostname) {
              host = authority.hostname;
            }
            if (isLocal(host, LOCAL_PORT)) {
              return origConnect.apply(this, arguments);
            }
          } catch (e) {}
          var session = origConnect.apply(this, arguments);
          try {
            if (session && !session.__gb_wrapped && typeof session.request === "function") {
              session.__gb_wrapped = true;
              var origRequest = session.request;
              session.request = function (headers, options) {
                try {
                  var p = pathFromH2(headers);
                  if (hitPath(p) && localUp && isCatalog(p)) {
                    return wrapH2CatalogMerge(origRequest.apply(this, arguments), p);
                  }
                } catch (e) {}
                return origRequest.apply(this, arguments);
              };
            }
          } catch (e) {}
          return session;
        };
      }
    } catch (e) {}
  } catch (e) {}
})();
