/** Overview route: the dashboard over the loaded workspace data; product picks open the preview on narrow screens. */
import { OverviewView } from "./OverviewView";
import { useStudio } from "../shell/StudioContext";

export default function OverviewRoute() {
  const { data, intent, onProduct, setPreviewOpen } = useStudio();
  return (
    <OverviewView
      data={data!}
      onIntent={intent}
      onProduct={(id) => {
        onProduct(id);
        if (window.matchMedia("(max-width:1024px)").matches)
          setPreviewOpen(true);
      }}
    />
  );
}
