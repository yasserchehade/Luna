import type { NextConfig } from "next";

const backendOrigin = process.env.LUNA_WEB_BACKEND_ORIGIN ?? "http://127.0.0.1:8787";

const nextConfig: NextConfig = {
  async rewrites() {
    return [{ source: "/api/luna/:path*", destination: `${backendOrigin}/api/:path*` }];
  },
};

export default nextConfig;
