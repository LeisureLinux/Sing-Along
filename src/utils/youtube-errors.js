/**
 * Work out what a failed YouTube download was caused by.
 *
 * `yt-dlp` names the symptom rather than the fix, and the fix is almost
 * always the cookies or the proxy. A user who is told "Sign in to confirm
 * you're not a bot" has no way to know that the file on disk is the problem,
 * so the cause is turned into a sentence about what to do.
 *
 * The order matters: the more specific causes are checked first, because
 * their messages often contain the general ones. "Unable to connect to
 * proxy" is a connection failure, and a connection failure can also be a
 * rate limit, so the proxy is checked first.
 */

/** The causes, in the order they are checked. */
const CAUSES = [
  {
    // The cookies are the most common cause by a wide margin: a browser
    // rotates them without warning, and a rotated cookie is answered with
    // this message rather than with a refusal.
    id: 'cookies',
    test: message =>
      message.includes('not a bot') ||
      message.includes('sign in to confirm') ||
      message.includes('login required') ||
      message.includes('cookies are no longer valid') ||
      message.includes('account cookies'),
  },
  {
    id: 'proxy',
    test: message =>
      message.includes('proxy') ||
      message.includes('unable to connect') ||
      message.includes('connection refused') ||
      message.includes('connection reset') ||
      message.includes('timed out') ||
      message.includes('could not resolve'),
  },
  {
    id: 'tooMany',
    test: message => message.includes('429') || message.includes('too many requests'),
  },
  {
    id: 'unavailable',
    test: message =>
      message.includes('unavailable') ||
      message.includes('private video') ||
      message.includes('members-only') ||
      message.includes('removed by the uploader'),
  },
  {
    id: 'noBinary',
    test: message =>
      message.includes('yt-dlp was not found') ||
      message.includes('does not exist') ||
      message.includes('cannot run'),
  },
]

/**
 * The cause of a failure, as a key the caller can translate.
 *
 * Returns 'unknown' when the message is not one of the known ones, because
 * a message nobody recognises may still mean something to the person who
 * reads it — so it is shown as it is.
 */
export const classifyDownloadError = error => {
  const message = String(error || '').toLowerCase()

  for (const cause of CAUSES) {
    if (cause.test(message)) {
      return cause.id
    }
  }

  return 'unknown'
}
