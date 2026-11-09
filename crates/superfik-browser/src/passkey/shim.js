(function () {
  var c = navigator.credentials, send = window.__BINDING__;
  if (!c || !send || !window.PublicKeyCredential || window.__ANSWER__) return;
  try { delete window.__BINDING__; } catch (_) {}
  var get = c.get.bind(c), create = c.create.bind(c), next = 1, pending = {};
  function enc(b) {
    var u = b instanceof ArrayBuffer ? new Uint8Array(b) : new Uint8Array(b.buffer, b.byteOffset, b.byteLength), s = '';
    for (var i = 0; i < u.length; i++) s += String.fromCharCode(u[i]);
    return btoa(s).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
  }
  function dec(t) {
    var s = atob(t.replace(/-/g, '+').replace(/_/g, '/')), u = new Uint8Array(s.length);
    for (var i = 0; i < s.length; i++) u[i] = s.charCodeAt(i);
    return u.buffer;
  }
  function own(o, k, v) { Object.defineProperty(o, k, { value: v, enumerable: true }); }
  function credential(id, response, json) {
    var p = Object.create(PublicKeyCredential.prototype);
    own(p, 'id', id);
    own(p, 'rawId', dec(id));
    own(p, 'type', 'public-key');
    own(p, 'authenticatorAttachment', 'platform');
    own(p, 'response', response);
    own(p, 'getClientExtensionResults', function () { return {}; });
    own(p, 'toJSON', function () {
      return { id: id, rawId: id, type: 'public-key', authenticatorAttachment: 'platform', response: json,
        clientExtensionResults: {} };
    });
    return p;
  }
  function signed(a) {
    var r = Object.create(AuthenticatorAssertionResponse.prototype);
    own(r, 'clientDataJSON', dec(a.client_data));
    own(r, 'authenticatorData', dec(a.authenticator_data));
    own(r, 'signature', dec(a.signature));
    own(r, 'userHandle', a.user_handle ? dec(a.user_handle) : null);
    return credential(a.credential_id, r, { clientDataJSON: a.client_data, authenticatorData: a.authenticator_data,
      signature: a.signature, userHandle: a.user_handle || null });
  }
  function created(a) {
    var r = Object.create(AuthenticatorAttestationResponse.prototype), transports = ['hybrid', 'internal'];
    own(r, 'clientDataJSON', dec(a.client_data));
    own(r, 'attestationObject', dec(a.attestation_object));
    own(r, 'getAuthenticatorData', function () { return dec(a.authenticator_data); });
    own(r, 'getPublicKey', function () { return dec(a.public_key); });
    own(r, 'getPublicKeyAlgorithm', function () { return -7; });
    own(r, 'getTransports', function () { return transports.slice(); });
    return credential(a.credential_id, r, { clientDataJSON: a.client_data, attestationObject: a.attestation_object,
      authenticatorData: a.authenticator_data, publicKey: a.public_key, publicKeyAlgorithm: -7,
      transports: transports });
  }
  Object.defineProperty(window, '__ANSWER__', { value: function (id, reply) {
    var p = pending[id];
    if (!p) return;
    delete pending[id];
    if (reply.signed) p.resolve(signed(reply.signed));
    else if (reply.created) p.resolve(created(reply.created));
    else if (reply.elsewhere) p.fallback().then(p.resolve, p.reject);
    else p.reject(new DOMException('The operation either timed out or was not allowed.', 'NotAllowedError'));
  } });
  function aborted(signal) { return signal.reason || new DOMException('The operation was aborted.', 'AbortError'); }
  function ask(options, message, fallback) {
    var signal = options.signal;
    if (signal && signal.aborted) return Promise.reject(aborted(signal));
    var id = next++;
    message.request = id;
    return new Promise(function (resolve, reject) {
      pending[id] = { resolve: resolve, reject: reject, fallback: fallback };
      if (signal) signal.addEventListener('abort', function () {
        if (pending[id]) { delete pending[id]; reject(aborted(signal)); }
      });
      send(JSON.stringify(message));
    });
  }
  function ids(list) { return (list || []).map(function (x) { return enc(x.id); }); }
  c.get = function (options) {
    var pk = options && options.publicKey;
    if (!pk) return get(options);
    return ask(options, { kind: 'get', conditional: options.mediation === 'conditional', rpId: pk.rpId || '',
      challenge: enc(pk.challenge), allowed: ids(pk.allowCredentials) }, function () { return get(options); });
  };
  c.create = function (options) {
    var pk = options && options.publicKey;
    if (!pk || !pk.user) return create(options);
    return ask(options, { kind: 'create', rpId: (pk.rp && pk.rp.id) || '', rpName: (pk.rp && pk.rp.name) || '',
      challenge: enc(pk.challenge), user: { id: enc(pk.user.id), name: pk.user.name || '',
        displayName: pk.user.displayName || '' }, excluded: ids(pk.excludeCredentials),
      algorithms: (pk.pubKeyCredParams || []).map(function (p) { return p.alg; }) },
      function () { return create(options); });
  };
})()
