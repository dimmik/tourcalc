import React from 'react';
import ReactDOM from 'react-dom';
import AppState from './appstate.jsx'

import Dialog from '@material-ui/core/Dialog';
import DialogTitle from '@material-ui/core/DialogTitle';
import DialogContent from '@material-ui/core/DialogContent';
import DialogActions from '@material-ui/core/DialogActions';

export default class PersonForm extends React.Component {
    constructor(props) {
        super(props);
        this.state = {
            dialogOpen: props.open
        }
        if (props.name != null) this.name = props.name
        if (props.weight != null) this.weight = props.weight
        //alert('open: ' + props.open)
    }
    name = ""
    weight = 100
    render() {
        return (
            <span>
                <span style={{ cursor: "pointer"}} onClick={() => this.setState({ dialogOpen: true })}>
                    {this.props.buttonText}
                </span>
            <Dialog aria-labelledby="customized-dialog-title" open={this.state.dialogOpen}>
                <DialogTitle id="customized-dialog-title">Add Person</DialogTitle>
                    <DialogContent>
                        <form onSubmit={(event) => {
                            event.preventDefault();
                            //alert('sending')
                            AppState.addPerson(this.props.app, this.props.tourid, { name: this.name, weight: this.weight })
                                .then(AppState.loadTour(this.props.app, this.props.tourid))
                            }}>
                            <p>name:</p>
                            <input
                                type='text'
                                onChange={(e) => this.name = event.target.value}
                                defaultValue={this.name}
                            />
                            <p>weight %:</p>
                            <input
                                type='number'
                                onChange={(e) => this.weight = event.target.value}
                                defaultValue={this.weight}
                            />
                        </form>
                </DialogContent>
                <DialogActions>
                        <button color="primary" onClick={() => {
                            (  this.props.mode === "add"
                                ? AppState.addPerson(this.props.app, this.props.tourid, { name: this.name, weight: this.weight })
                                : AppState.editPerson(this.props.app, this.props.tourid, { guid: this.props.guid, name: this.name, weight: this.weight })
                            )
                                .then(this.setState({ dialogOpen: false }))
                                .then(() => { AppState.loadTour(this.props.app, this.props.tourid) })

                        }}>{this.props.actionButtonText}</button>
                        <button onClick={() => { this.setState({ dialogOpen: false }) }}>Cancel</button>
                </DialogActions>
            </Dialog>
            </span>
        )
    }
}
