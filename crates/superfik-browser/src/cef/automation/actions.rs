//! Read/actionability checks on the native DOM node, including closed roots.
pub(super) const CHECK: &str = r#"function(select) {
  const e = this;
  if (!e.isConnected || e.disabled || e.getAttribute('aria-disabled') === 'true')
    throw new Error('Element is detached or disabled; take a new snapshot');
  const r = e.getBoundingClientRect(), s = getComputedStyle(e);
  if (!r.width || !r.height || s.visibility === 'hidden' || s.display === 'none')
    throw new Error('Element is not visible');
  const root = e.getRootNode(), hit = root.elementFromPoint(r.x+r.width/2,r.y+r.height/2);
  if (!hit || (hit !== e && !e.contains(hit))) throw new Error('Element is covered by another element');
  if (select) {
    if (e.readOnly || !(e instanceof HTMLInputElement || e instanceof HTMLTextAreaElement || e.isContentEditable))
      throw new Error('Element cannot receive text');
    e.focus();
    if (typeof e.select === 'function') e.select();
    else { const s = getSelection(), r = document.createRange(); r.selectNodeContents(e); s.removeAllRanges(); s.addRange(r); }
  }
  return { file: e instanceof HTMLInputElement && e.type === 'file' };
}"#;
