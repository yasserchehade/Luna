import { redirect } from "next/navigation";

export default function Home() {
  redirect("/prototype/web-first?variant=A");
}
