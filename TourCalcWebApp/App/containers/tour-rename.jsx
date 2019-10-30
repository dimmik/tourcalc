import React from 'react';
import ReactDOM from 'react-dom';
import AppState from './appstate.jsx'

import Dialog from '@material-ui/core/Dialog';
import DialogTitle from '@material-ui/core/DialogTitle';
import DialogContent from '@material-ui/core/DialogContent';
import DialogActions from '@material-ui/core/DialogActions';

export default class TourNameEdit extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            dialogOpen: props.open,
            tourid: props.tourid
        }
        this.name = props.name
    }

    name = ""
    code = ""

    render() {
        return (
            <span key={this.state.tourid}>
                <span style={{ cursor: 'pointer', fontSize: "xx-small"}} onClick={() => this.setState({ dialogOpen: true })}>
                    {this.props.buttonText}
                </span>
                <Dialog fullWidth={true} aria-labelledby="customized-dialog-title" open={this.state.dialogOpen} onClose={() => { this.setState({ dialogOpen: false }) }}>
                <DialogTitle id="customized-dialog-title">Change Tour Name</DialogTitle>
                    <DialogContent>
                        <form onSubmit={(event) => {
                            event.preventDefault();
                            }}>
                            <p>Tour Name:</p>
                            <input
                                type='text'
                                onChange={(e) => this.name = event.target.value}
                                defaultValue={this.name}
                            />
                            {this.props.authData.type === 'Master' ? (
                                <span><p>Tour Code:</p>
                                <input
                                    type='text'
                                    onChange={(e) => this.code = event.target.value}
                                    defaultValue=""
                                /></span>) : <span />}
                        </form>
                </DialogContent>
                <DialogActions>
                        <button color="primary" onClick={() => {
                            AppState.changeTourName(this.props.app, this.props.tourid, this.name, this.code)
                                //.then(res => { alert('res: ' + res) }, error => { alert('error: ' + error) })
                                .then(() => this.setState({ dialogOpen: false }))
                                .then(() => { this.props.app.loadTours() })

                        }}>{this.props.actionButtonText}</button>
                        <button onClick={() => { this.setState({ dialogOpen: false }) }}>Cancel</button>
                </DialogActions>
            </Dialog>
            </span>
        )
    }
}
