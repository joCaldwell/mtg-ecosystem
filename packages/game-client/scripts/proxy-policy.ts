/** Local delivery policy agreed with the native serving owner. This config is
 * server-side only. No bootstrap input ever selects a proxy destination. */
export function upstreamOrigin(value: string): string {
  if (!/^http:\/\/(127\.0\.0\.1|\[::1\]):[1-9][0-9]{0,4}$/.test(value) || /\s/.test(value)) throw new Error('Expected an explicit numeric loopback upstream');
  const url = new URL(value);
  if (`http://${url.hostname}:${url.port || '80'}` !== value) throw new Error('Noncanonical loopback upstream');
  return value;
}
export function localRequestAllowed(host: string | undefined, origin: string | undefined, site: string | undefined, port: number): boolean {
  return host === `127.0.0.1:${port}` && (origin === undefined || origin === `http://${host}`)
    && (site === undefined || site === 'same-origin' || site === 'none');
}
export function readRouteAllowed(method: string | undefined, target: string): boolean {
  if (method !== 'GET') return false;
  // Do not normalize dot segments, encoded slashes, fragments or aliases into an
  // allowed route. The native/SDK validators own the bounded event query itself.
  return /^\/v1\/matches\/[0-9a-f]{1,64}\/(manifest|resume|presentation)$/.test(target)
    || /^\/v1\/matches\/[0-9a-f]{1,64}\/events\?after=[0-9]{1,20}&limit=[0-9]{1,2}&wait_ms=[0-9]{1,5}$/.test(target)
    || /^\/presentation\/[0-9a-f]{64}\.json$/.test(target);
}
