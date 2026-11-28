/** The one renderer of catalog documents: validates, renders admitted nodes with kit components and shows localized fallbacks for refusals. */
import { Component, useEffect, useMemo, type ReactNode } from "react";
import { Notice } from "../ui";
import { refusalText, catalogText } from "./catalog-i18n";
import {
  validateDocument,
  type Admission,
  type BoundAction,
  type Problem,
  type ProblemCode,
  type ValidNode,
} from "./validate";

/** What the host receives when a document's control reports an event bound to an admitted action. */
export type DocumentAction = BoundAction & {
  event: string;
  node: string;
  payload?: unknown;
};

export type CatalogViewProps = {
  document: unknown;
  admission: Admission;
  onAction?: (action: DocumentAction) => void;
  /** Called with every refusal, for diagnostics; refusals are also shown in place. */
  onProblems?: (problems: Problem[]) => void;
};

function Refused({
  code,
  locale,
  id,
}: {
  code: ProblemCode | "render";
  locale: string;
  id: string;
}) {
  return (
    <div className="vy-doc-refused" data-refusal={code} data-node={id}>
      <Notice tone="warning" title={catalogText(locale)("refusedTitle")}>
        {refusalText(locale, code)}
      </Notice>
    </div>
  );
}

/** Keeps a failing component from taking the host down; its place shows the fallback. */
class Boundary extends Component<
  { id: string; locale: string; children: ReactNode },
  { failed: boolean }
> {
  state = { failed: false };
  static getDerivedStateFromError() {
    return { failed: true };
  }
  render() {
    return this.state.failed ? (
      <Refused code="render" locale={this.props.locale} id={this.props.id} />
    ) : (
      this.props.children
    );
  }
}

/** Calls a render function as a component, so its failures stay inside the node's boundary. */
function Deferred({ render }: { render: () => ReactNode }) {
  return <>{render()}</>;
}

function renderNode(
  node: ValidNode,
  admission: Admission,
  onAction: CatalogViewProps["onAction"],
): ReactNode {
  if (node.kind === "refused")
    return (
      <Refused
        key={node.id}
        code={node.code}
        locale={admission.locale}
        id={node.id}
      />
    );
  const slots = Object.fromEntries(
    Object.entries(node.slots).map(([slot, children]) => [
      slot,
      children.map((child) => renderNode(child, admission, onAction)),
    ]),
  );
  if (node.kind === "host")
    return (
      <Boundary key={node.id} id={node.id} locale={admission.locale}>
        <Deferred render={() => node.host.render({ slots, id: node.id })} />
      </Boundary>
    );
  const { entry, action } = node;
  const slotProps = Object.fromEntries(
    Object.entries(node.slots).map(([slot, children]) => [
      slot,
      children.map((child) => (child.kind === "entry" ? child.props : {})),
    ]),
  );
  const emit = (event: string, payload?: unknown) => {
    const rule = entry.events?.[event];
    if (!action || !rule || !onAction) return;
    if (rule.payload && !rule.payload.safeParse(payload).success) return;
    onAction({ ...action, event, node: node.id, payload });
  };
  return (
    <Boundary key={node.id} id={node.id} locale={admission.locale}>
      <div
        className="vy-doc-node"
        data-component={entry.name}
        data-node={node.id}
      >
        <Deferred
          render={() =>
            entry.render({
              props: node.props,
              slots,
              slotProps,
              emit,
              bound: (event) =>
                Boolean(action && onAction && entry.events?.[event]),
              locale: admission.locale,
              id: node.id,
            })
          }
        />
      </div>
    </Boundary>
  );
}

/**
 * Renders a `veyndo-ui/1`, A2UI v1.0 or OpenUI Lang document for one surface. Every
 * node is checked against the catalog and the admission first; nothing the host did
 * not admit renders, and each refusal shows a localized notice in its place.
 */
export function CatalogView({
  document,
  admission,
  onAction,
  onProblems,
}: CatalogViewProps) {
  const validation = useMemo(
    () => validateDocument(document, admission),
    [document, admission],
  );
  useEffect(() => {
    if (validation.problems.length) onProblems?.(validation.problems);
  }, [validation, onProblems]);
  if (!validation.ok) {
    const code = validation.problems[0].code;
    return (
      <div
        className="vy-doc vy-doc-refused"
        data-refusal={code}
        data-surface={admission.surface}
      >
        <Notice
          tone="warning"
          title={catalogText(admission.locale)("documentTitle")}
        >
          {refusalText(admission.locale, code)}
        </Notice>
      </div>
    );
  }
  return (
    <div
      className="vy-doc"
      data-surface={admission.surface}
      data-format={validation.format}
    >
      {renderNode(validation.root, admission, onAction)}
    </div>
  );
}
