/**
 * pnpm versions differ in whether `pnpm run <script> -- <arg>` preserves the
 * separator. Accept both forms while still rejecting ambiguous invocations.
 */
export function parseBinaryPathArgument(arguments_) {
  const binaryPaths = arguments_.filter((argument) => argument !== "--");
  if (binaryPaths.length > 1) {
    throw new Error(
      `Expected at most one binary path, received: ${binaryPaths.join(" ")}`,
    );
  }
  return binaryPaths[0];
}
