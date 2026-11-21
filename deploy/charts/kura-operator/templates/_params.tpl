{{/*
This operator's parameters for the kit's library chart (weebo-operator-lib,
see its README): everything templates/operator.yaml renders that is not a
value.
*/}}
{{- define "operator.params" -}}
group: kura.weebo.io
versions: [v1alpha1]
# Every kind the operator watches (ClusterRole).
kinds:
  - kuratenants
  - kuravaults
  - kurainstances
# The namespaced kinds: what a team may manage (rbac.namespacedExample).
namespacedKinds:
  - kuravaults
# Kinds the admission webhook checks (RFC 0009 §4.3): a vault's tenant and
# namespace, one instance per cluster. Tenants carry no cross-object rule.
admitted:
  - kuravaults
  - kurainstances
# No Secret is read or written cluster-wide; a KuraInstance's tls.caSecretRef
# must live in the release namespace and be listed in caSecrets.
clusterSecrets: false
releaseSecrets: {{ toJson .Values.caSecrets }}
# The operator mints its own token for Kura through the TokenRequest API
# (audience: the KuraInstance's auth.serviceAccountToken.audience), for its
# own ServiceAccount only (RFC 0009, plan D18).
clusterRules:
  - apiGroups: [""]
    resources: ["serviceaccounts/token"]
    resourceNames: [{{ include "weebo-operator.serviceAccountName" . | quote }}]
    verbs: ["create"]
# The operator's own environment variables.
env:
  - name: KURA_SERVICE_ACCOUNT
    valueFrom:
      fieldRef:
        fieldPath: spec.serviceAccountName
egress:
  - comment: Kura instances.
    rules: {{ toJson .Values.networkPolicy.egress.kura }}
# Alerts on the operator's own metrics, appended to the kit's.
alerts: []
{{- end -}}
