{{- define "kura.name" -}}{{ .Release.Name }}{{- end -}}

{{- define "kura.labels" -}}
app.kubernetes.io/name: kura
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
helm.sh/chart: {{ .Chart.Name }}-{{ .Chart.Version }}
{{- end -}}

{{/* The server's pods; jobs carry another component, so the Services, the
PodDisruptionBudget and the NetworkPolicy never count them. */}}
{{- define "kura.selector" -}}
app.kubernetes.io/name: kura
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/component: server
{{- end -}}

{{- define "kura.jobLabels" -}}
app.kubernetes.io/name: kura
app.kubernetes.io/instance: {{ .root.Release.Name }}
app.kubernetes.io/component: {{ .component }}
{{- end -}}

{{- define "kura.image" -}}
{{- if .Values.image.digest -}}
{{ .Values.image.repository }}@{{ .Values.image.digest }}
{{- else -}}
{{ .Values.image.repository }}:{{ .Values.image.tag | default .Chart.AppVersion }}
{{- end -}}
{{- end -}}

{{/* The non-root, read-only container every workload of the chart runs as. */}}
{{- define "kura.securityContext" -}}
allowPrivilegeEscalation: false
readOnlyRootFilesystem: true
runAsNonRoot: true
runAsUser: 65532
runAsGroup: 65532
capabilities: { drop: [ALL] }
seccompProfile: { type: RuntimeDefault }
{{- end -}}

{{/* The owner's database URL, for migrations. */}}
{{- define "kura.ownerUrl" -}}
{{- if .Values.migrate.databaseUrlSecret.name -}}
secretKeyRef: { name: {{ .Values.migrate.databaseUrlSecret.name }}, key: {{ .Values.migrate.databaseUrlSecret.key }} }
{{- else if .Values.cnpg.enabled -}}
secretKeyRef: { name: {{ .Values.cnpg.name }}-app, key: uri }
{{- else -}}
{{- fail "set migrate.databaseUrlSecret.name, or cnpg.enabled" -}}
{{- end -}}
{{- end -}}

{{/* `files` or `operator`: where the documents come from (RFC 0009). */}}
{{- define "kura.documentsSource" -}}
{{- $d := default dict .Values.server.documents -}}
{{- default "files" $d.source -}}
{{- end -}}
